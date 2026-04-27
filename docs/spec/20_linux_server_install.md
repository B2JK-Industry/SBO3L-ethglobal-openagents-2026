# Linux Server Installation Guide

> **Účel:** Konkrétny step-by-step návod, ako nainštalovať `mandate` na vlastný Linux server. Pokrýva 3 deployment profily (dev / production-HSM / production-TEE).
>
> **Cieľová OS:** Ubuntu 24.04 LTS (server, minimal install). Iné distrá v §10.

---

## §1 Hardware requirements

### §1.1 Profil "DEV / Hackathon Demo"
- CPU: čokoľvek x86_64 alebo ARM64 (Raspberry Pi 5 funguje)
- RAM: 2 GB minimum
- Disk: 10 GB
- Network: ethernet alebo Wi-Fi
- USB: 1 voľný port (pre HW kill switch ak demonujeme)

### §1.2 Profil "PRODUCTION-HSM"
- CPU: x86_64 alebo ARM64; mini-PC / NUC postačujúce
- RAM: 4 GB
- Disk: 64 GB SSD (LUKS-encrypted)
- TPM 2.0 (discrete chip alebo fTPM v firmware)
- USB: 2 voľné porty (HSM + voliteľne kill switch)
- HSM: **Nitrokey HSM 2** (~€100, primary) ALEBO **YubiHSM 2** ($650, secondary)

### §1.3 Profil "PRODUCTION-TEE" (cieľová architektúra)
- CPU: **Intel Xeon 5th-gen Emerald Rapids alebo 6th-gen Granite Rapids** s TDX, ALEBO **AMD EPYC Genoa/Bergamo/Turin** so SEV-SNP
- Alternatíva: Granite Rapids-WS (LGA 4710 workstation, 2026), prvý dostupný TDX mimo DC SKU
- Cloud: Azure DCesv5/ECesv5 (TDX), GCP `c3-standard --confidential-instance-type=TDX`, Hetzner SEV-SNP node
- RAM: 16 GB minimum (TEE má overhead)
- Disk: 128 GB NVMe (LUKS + TPM-bound unlock)
- TPM 2.0 (oddelený od TEE, používa sa pre LUKS)
- HSM: voliteľne (defense-in-depth nad TEE)

**WATCH:** Consumer Intel Core CPUs nemajú TDX. Nepokúšaj sa o TEE profil na bežnom desktope.

---

## §2 Pre-install — OS hardening

### §2.1 Inštalácia Ubuntu 24.04 LTS

Stiahni Ubuntu 24.04.x LTS Server ISO (`https://ubuntu.com/download/server`). Boot z USB. Inštalátor:

1. **Storage:** Manual partitioning.
   - `/boot` 1 GB ext4 (unencrypted)
   - `/boot/efi` 512 MB FAT32
   - **LUKS2 volume** zaberajúci zvyšok disku (heslo dočasné, neskôr nahradíme TPM2 unlock)
   - V LUKS: LVM PV → VG `vg0` → LVs:
     - `lv_root` 16 GB ext4 → `/`
     - `lv_var` 16 GB ext4 → `/var`
     - `lv_mandate` zvyšok ext4 → `/var/lib/mandate` (mount options `nodev,nosuid,noexec`)
2. **User:** `mandate-admin` (sudoer); SSH password disabled.
3. **Profile:** Minimal install. **NIE** docker / snap / flatpak v inštalátore.
4. **Reboot.**

### §2.2 Initial OS hardening

```bash
# Patch & reboot
sudo apt update && sudo apt full-upgrade -y
sudo reboot

# Kernel ≥ 6.7 needed for configfs-tsm (TDX); Ubuntu 24.04 uses 6.8 by default.
uname -r  # expect 6.8+

# SSH hardening
sudo tee /etc/ssh/sshd_config.d/99-hardening.conf << 'EOF'
PasswordAuthentication no
PermitRootLogin no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
AllowUsers mandate-admin
ClientAliveInterval 300
ClientAliveCountMax 2
MaxAuthTries 3
LoginGraceTime 30
EOF
sudo systemctl restart ssh

# UFW firewall
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow from 192.168.0.0/16 to any port 22 proto tcp
# Vault REST/gRPC porty NIE sú externe expozné (loopback only).
sudo ufw enable

# Disable services we won't need
sudo systemctl disable --now snapd unattended-upgrades.service
# (Nahradíme manual upgrade flow s SHA verifikáciou.)

# Time sync (critical for TEE attestation freshness + x402 expiry)
sudo apt install -y chrony
sudo systemctl enable --now chrony

# Audit framework
sudo apt install -y auditd audispd-plugins
sudo systemctl enable --now auditd

# AppArmor (defaults to enforcing on Ubuntu)
sudo aa-status
```

### §2.3 LUKS2 + TPM2 unattended unlock

```bash
sudo apt install -y systemd-cryptenroll tpm2-tools clevis

# Identify LUKS device (e.g. /dev/nvme0n1p3)
sudo cryptsetup luksDump /dev/nvme0n1p3

# Enroll TPM2 with PCR 7 (Secure Boot state) + 11 (Unified Kernel Image) + 14 (MOK)
sudo systemd-cryptenroll --tpm2-device=auto \
                          --tpm2-pcrs=7+11+14 \
                          --tpm2-with-pin=yes \
                          /dev/nvme0n1p3

# Update /etc/crypttab
echo "vault UUID=$(sudo blkid -s UUID -o value /dev/nvme0n1p3) none tpm2-device=auto,tpm2-pcrs=7+11+14,headless" | sudo tee -a /etc/crypttab

# Regenerate initramfs
sudo update-initramfs -u -k all

# Test reboot — disk should unlock without prompting (only if Secure Boot state unchanged)
```

**WATCH:**
- **Nepoužívaj PCR 0** (firmware hash) — BIOS update rebrickne LUKS.
- Vyžaduj PIN aspoň pre admin reboot (`--tpm2-with-pin=yes`).
- Záložný heslo passphrase je stále zapísaný v LUKS slot 0 — uschovaj v offline backup.

### §2.4 Disable swap (defense-in-depth pre key material)

```bash
sudo swapoff -a
sudo sed -i 's/^[^#].*swap.*$/#&/' /etc/fstab
# Verify:
free -h  # Swap row should be 0
```

---

## §3 Install mandate binary

### §3.1 Profil "DEV"

```bash
# Latest release from GitHub
MANDATE_VERSION="v0.1.0"  # adjust
curl -L -O "https://github.com/<org>/mandate/releases/download/${MANDATE_VERSION}/mandate-${MANDATE_VERSION}-x86_64-linux-musl.tar.gz"
curl -L -O "https://github.com/<org>/mandate/releases/download/${MANDATE_VERSION}/mandate-${MANDATE_VERSION}-x86_64-linux-musl.tar.gz.cosign-bundle"

# Verify signature with cosign v3
cosign verify-blob \
  --bundle mandate-${MANDATE_VERSION}-x86_64-linux-musl.tar.gz.cosign-bundle \
  --certificate-identity "https://github.com/<org>/mandate/.github/workflows/release.yml@refs/tags/${MANDATE_VERSION}" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  mandate-${MANDATE_VERSION}-x86_64-linux-musl.tar.gz

# Extract & install
tar xzf mandate-${MANDATE_VERSION}-x86_64-linux-musl.tar.gz
sudo install -o root -g root -m 0755 mandate /usr/bin/mandate
mandate --version
```

### §3.2 Profil "PRODUCTION" — `.deb` package

```bash
curl -L -O "https://github.com/<org>/mandate/releases/download/${MANDATE_VERSION}/mandate_${MANDATE_VERSION}_amd64.deb"
curl -L -O "https://github.com/<org>/mandate/releases/download/${MANDATE_VERSION}/mandate_${MANDATE_VERSION}_amd64.deb.cosign-bundle"

# Verify
cosign verify-blob \
  --bundle mandate_${MANDATE_VERSION}_amd64.deb.cosign-bundle \
  --certificate-identity "..." \
  --certificate-oidc-issuer "..." \
  mandate_${MANDATE_VERSION}_amd64.deb

# Install — creates user `mandate`, dirs, systemd unit (disabled by default)
sudo dpkg -i mandate_${MANDATE_VERSION}_amd64.deb
```

`.deb` postinst:
- Vytvorí systémového usera `mandate` (uid < 1000)
- Vytvorí adresáre s permissions:
  - `/etc/mandate/` (`0750 root:mandate`)
  - `/var/lib/mandate/` (`0700 mandate:mandate`)
  - `/var/log/mandate/` (`0750 mandate:mandate`)
  - `/run/mandate/` (vytvára systemd-tmpfiles, `0750 mandate:mandate`)
- Inštaluje systemd unit `/usr/lib/systemd/system/mandate.service` (NIE enabled)
- Inštaluje AppArmor profil `/etc/apparmor.d/usr.bin.mandate`

---

## §4 First-time setup wizard

```bash
sudo -u mandate mandate init --interactive
```

Wizard prejde cez:

1. **Bootstrap admin pubkey** — vlož verejný kľúč prvého admina (Ed25519 hex). Doporučené generovať na separátnom HW (YubiKey OpenPGP, Nitrokey HSM 2 admin slot).
2. **Vault private CA** — generuje sa nový Ed25519 keypair pre podpisovanie agentových mTLS cert. V production-HSM profile sa kľúč generuje v HSM.
3. **Audit signer key** — separátny Ed25519 keypair pre podpisovanie audit eventov.
4. **Decision signer key** — separátny Ed25519 keypair pre internal decision token signing.
5. **Default config** — vytvorí `/etc/mandate/mandate.toml` s baseline (mode=dev). Treba edit pred enabledením v production.
6. **Default policy** — uloží `policy://default-deny-all` ako baseline.

Po wizardovi vault NIE je zapnutý. Treba edit configu, potom `systemctl enable --now`.

---

## §5 Configuration — `/etc/mandate/mandate.toml`

Plný reference v `17_interface_contracts.md §1`. Minimálny production config:

```toml
[server]
mode = "production"
unix_socket_path = "/run/mandate/mandate.sock"
unix_socket_owner = "mandate"
unix_socket_perms = "0600"
tcp_listen = "127.0.0.1:8730"   # NIKDY 0.0.0.0
http2 = true
max_request_bytes = 65536
shutdown_grace_seconds = 30

[storage]
db_path = "/var/lib/mandate/mandate.db"
wal_mode = true
journal_size_limit_mb = 64

[signing]
default_backend = "hsm_pkcs11"   # production
allow_dev_key = false
attestation_required_default = true

[[signing.keys]]
id = "operational-key"
backend = "hsm_pkcs11"
backend_config = { module = "/usr/lib/x86_64-linux-gnu/pkcs11/opensc-pkcs11.so", slot = 0, label = "operational" }
purpose = "operational"
attestation_required = true

[[signing.keys]]
id = "treasury-key"
backend = "hsm_pkcs11"
backend_config = { module = "/usr/lib/x86_64-linux-gnu/pkcs11/opensc-pkcs11.so", slot = 1, label = "treasury" }
purpose = "treasury"
multisig_required = true

[audit]
hash_algorithm = "sha256"
audit_signer_key_path = "/var/lib/mandate/keys/audit-signer.age"
on_chain_anchor_enabled = true
anchor_chain = "base"
anchor_period_hours = 24

[emergency]
hw_killswitch_device = "/dev/input/event5"   # nahradiť skutočným event N
killswitch_double_press_window_ms = 1000
auto_freeze_anomaly_threshold = 0.95

[telemetry]
log_format = "json"
log_level = "info"
metrics_listen = "127.0.0.1:9091"
otlp_endpoint = ""              # disable until you have OTel collector

[[chains]]
id = "base"
chain_id = 8453
rpc_endpoints = [
  "https://mainnet.base.org",
  "https://base.publicnode.com",
  "https://1rpc.io/base"
]
rpc_quorum_min = 2
simulator_endpoint = "https://mainnet.base.org"
block_explorer_url = "https://basescan.org"

[[chains.tokens]]
symbol = "USDC"
address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
decimals = 6
is_stablecoin = true
```

Validuj pred štartom:

```bash
sudo -u mandate mandate config check --config /etc/mandate/mandate.toml
sudo -u mandate mandate config check --production --config /etc/mandate/mandate.toml
```

---

## §6 systemd unit (hardened)

`/usr/lib/systemd/system/mandate.service`:

```ini
[Unit]
Description=mandate — local payment vault for AI agents
Documentation=https://github.com/<org>/mandate
After=network-online.target tpm2-abrmd.service
Wants=network-online.target

[Service]
Type=notify
NotifyAccess=main
WatchdogSec=30s
Restart=on-failure
RestartSec=10s

User=mandate
Group=mandate
UMask=0077

ExecStart=/usr/bin/mandate serve --config /etc/mandate/mandate.toml
ExecReload=/bin/kill -HUP $MAINPID

# Credentials (TPM-encrypted passphrases)
LoadCredentialEncrypted=audit_signer_pass:/etc/credstore.encrypted/audit_signer_pass
LoadCredentialEncrypted=decision_signer_pass:/etc/credstore.encrypted/decision_signer_pass

# Hardening (per 19_knowledge_base.md §6.1)
NoNewPrivileges=yes
ProtectSystem=strict
ReadWritePaths=/var/lib/mandate /var/log/mandate /run/mandate
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=no
DevicePolicy=closed
DeviceAllow=/dev/tpmrm0 rw
DeviceAllow=char-usb_device rw
DeviceAllow=/dev/bus/usb rw
DeviceAllow=/dev/hidraw* rw
DeviceAllow=/dev/input/event5 r
PrivateNetwork=no
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
IPAddressDeny=any
IPAddressAllow=127.0.0.0/8
IPAddressAllow=::1/128
IPAddressAllow=mainnet.base.org
IPAddressAllow=base.publicnode.com
IPAddressAllow=1rpc.io
CapabilityBoundingSet=
AmbientCapabilities=
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources @mount @swap @reboot @debug @cpu-emulation @obsolete @raw-io @module
SystemCallArchitectures=native
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
ProtectProc=invisible
ProcSubset=pid
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
KeyringMode=private
RemoveIPC=yes
BindReadOnlyPaths=/etc/mandate

# Resource limits
LimitNOFILE=65536
LimitMEMLOCK=64K

[Install]
WantedBy=multi-user.target
```

Audit hardening score:

```bash
sudo systemd-analyze security mandate.service
# Target: < 1.0
```

---

## §7 HSM enrollment (Nitrokey HSM 2 / YubiHSM 2)

### §7.1 Nitrokey HSM 2 — install + enrollment

```bash
sudo apt install -y opensc opensc-pkcs11

# udev rules (per 19_knowledge_base.md §6.3)
sudo tee /etc/udev/rules.d/70-mandate-hsm.rules << 'EOF'
# Nitrokey HSM 2
SUBSYSTEM=="usb", ATTRS{idVendor}=="20a0", ATTRS{idProduct}=="4230", \
  GROUP="mandate", MODE="0660"
KERNEL=="hidraw*", ATTRS{idVendor}=="20a0", GROUP="mandate", MODE="0660"
EOF
sudo udevadm control --reload && sudo udevadm trigger

# Pripoj Nitrokey HSM 2 do USB.
# Initialize (pri prvom použití):
sudo -u mandate sc-hsm-tool --initialize --so-pin 3537363231383830 --pin 648219 \
                                --label "AgentVault"
# Generate operational key (slot 0):
sudo -u mandate pkcs11-tool --module /usr/lib/x86_64-linux-gnu/opensc-pkcs11.so \
                                  --login --pin 648219 \
                                  --keypairgen --key-type EC:secp256k1 \
                                  --label operational --id 01

# Generate treasury key (slot 1):
sudo -u mandate pkcs11-tool --module /usr/lib/x86_64-linux-gnu/opensc-pkcs11.so \
                                  --login --pin 648219 \
                                  --keypairgen --key-type EC:secp256k1 \
                                  --label treasury --id 02

# Verify enrollment
sudo -u mandate mandate keys list
# Expected: 2 keys (operational, treasury), backend=hsm_pkcs11
```

### §7.2 YubiHSM 2 — alternatíva

```bash
# Install yubihsm-shell + connector
sudo apt install -y yubihsm-shell

# Configure connector (yubihsm-connector daemon)
sudo systemctl enable --now yubihsm-connector

# Generate key via shell
yubihsm-shell -a generate-asymmetric-key -i 1 -l operational \
              --domains 1 --capabilities sign-ecdsa --algorithm ecp256k1

# Vault config: backend = "yubihsm_native" (separate backend type pre lepšiu attestation)
```

---

## §8 TPM enrollment for vault key wrap (production-TEE profil)

V profile TEE používame TPM ako secondary trust anchor (LUKS unlock + key wrap).

```bash
sudo apt install -y tpm2-tools tpm2-abrmd

# Verify TPM available
sudo tpm2_getcap properties-fixed | grep TPM2_PT_MANUFACTURER

# Generate persistent key sealed to PCRs (vault wrap key)
sudo -u mandate tpm2_createprimary -C o -c primary.ctx
sudo -u mandate tpm2_create -C primary.ctx \
                                  -u key.pub -r key.priv \
                                  -L policy.pcr -a "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|sign|decrypt"

# Use TPM-derived key to wrap age recipient for audit log encryption
# (Vault automates this via `mandate keys tpm-enroll --label audit-wrap`)
sudo -u mandate mandate keys tpm-enroll --label audit-wrap --pcrs 7+11
```

---

## §9 First start + smoke test

```bash
# Lint config
sudo -u mandate mandate config check --production

# Enable + start
sudo systemctl enable --now mandate.service

# Watch logs
sudo journalctl -u mandate -f

# Health check
sudo -u mandate mandate health
# Expected: status=ok, version=..., backend healthy

# Try to make a payment request (test agent)
sudo -u mandate mandate test-agent --send-mock-payment
```

---

## §10 Distribution-specific notes

### Fedora / RHEL 9
- Replace `apt` with `dnf`.
- AppArmor → SELinux. Use `mandate-selinux` package.
- udev rules under `/etc/udev/rules.d/` (same).

### Arch
- AUR package `mandate-bin` (mirror of GitHub release).
- AppArmor not default; install `apparmor` + enable.

### NixOS
- Flake at `flake:mandate`. NixOS module:
  ```nix
  services.mandate = {
    enable = true;
    settings = { /* mirrors mandate.toml */ };
  };
  ```

---

## §11 Upgrade procedure

**DON'T**: `apt upgrade mandate` blindly. Manual SHA verification:

```bash
NEW_VERSION="v0.2.0"

# Download new release + bundle
curl -L -O ".../mandate_${NEW_VERSION}_amd64.deb"
curl -L -O ".../mandate_${NEW_VERSION}_amd64.deb.cosign-bundle"

# Verify
cosign verify-blob --bundle ...

# Verify reproducible (optional)
bash scripts/verify-reproducible.sh mandate_${NEW_VERSION}_amd64.deb

# Stop vault, install, run migration check
sudo systemctl stop mandate
sudo dpkg -i mandate_${NEW_VERSION}_amd64.deb
sudo -u mandate mandate migrate check
sudo -u mandate mandate migrate apply
sudo systemctl start mandate

# Smoke test
sudo -u mandate mandate health
```

---

## §12 Backup procedure

Backup target: ALL of:
- `/etc/mandate/` (config + policies)
- `/var/lib/mandate/mandate.db` (state)
- `/var/lib/mandate/keys/*.age` (encrypted keys; passphrase backed up SEPARATELY)
- `/var/lib/mandate/audit/manifests/` (signed audit roots)
- HSM backup (vendor-specific; e.g. `sc-hsm-tool --create-dkek-share` for Nitrokey HSM 2)

```bash
# Daily backup script
sudo tee /usr/local/bin/mandate-backup << 'EOF'
#!/bin/bash
set -euo pipefail
BACKUP_DIR=/var/backups/mandate
TS=$(date +%Y%m%d-%H%M%S)
mkdir -p "$BACKUP_DIR"

# Stop vault for consistent snapshot
systemctl stop mandate

# Snapshot
tar czf "$BACKUP_DIR/mandate-${TS}.tar.gz" \
  /etc/mandate \
  /var/lib/mandate/mandate.db \
  /var/lib/mandate/mandate.db-wal \
  /var/lib/mandate/keys \
  /var/lib/mandate/audit/manifests

systemctl start mandate

# Encrypt to backup recipient
age -r age1bk... -o "$BACKUP_DIR/mandate-${TS}.tar.gz.age" \
                  "$BACKUP_DIR/mandate-${TS}.tar.gz"
shred -u "$BACKUP_DIR/mandate-${TS}.tar.gz"

# Sync to remote (e.g. S3 with object-lock, IPFS)
rclone copy "$BACKUP_DIR/mandate-${TS}.tar.gz.age" remote:mandate-backups/

# Retention
find "$BACKUP_DIR" -name "*.age" -mtime +30 -delete
EOF
sudo chmod +x /usr/local/bin/mandate-backup

# systemd timer
sudo tee /etc/systemd/system/mandate-backup.timer << 'EOF'
[Unit]
Description=Daily mandate backup
[Timer]
OnCalendar=daily
Persistent=true
[Install]
WantedBy=timers.target
EOF
sudo tee /etc/systemd/system/mandate-backup.service << 'EOF'
[Unit]
Description=mandate backup
[Service]
Type=oneshot
ExecStart=/usr/local/bin/mandate-backup
EOF
sudo systemctl enable --now mandate-backup.timer
```

---

## §13 Recovery — disk theft scenario

Ak je notebook ukradnutý (LUKS encrypted disk):

1. **Bez TPM-bound LUKS:** útočník má disk; ak má aj passphrase → access. Bez passphrase → no access.
2. **S TPM-bound LUKS (default):** útočník nedokáže odomknúť LUKS na inom HW (PCR mismatch).
3. **Ak HSM:** kľúč nebol nikdy na disku, je v HSM (ak útočník zoberie aj HSM, stále potrebuje SO PIN).

**Akcie po krádeži:**
1. **Z internetu:** ak ešte beží, `mandate emergency stop` cez admin signed payload (z mobilu / iného HW).
2. **On-chain:** smart account session keys revoke cez admin podpis na inom devices.
3. **Treasury rotate:** ak treasury kľúče v HSM, urob multisig rotation na nové kľúče.
4. **Audit:** stiahni posledný S3 backup audit logu, externý auditor verifikuje hash chain integrity.
5. **Deploy nový vault** na novom HW; restore from backup; nahraď kompromitované kľúče.

---

## §14 Monitoring

### §14.1 Local metrics

Vault exponuje Prometheus metrics na `127.0.0.1:9091/metrics`. Doporučené alarms:

```yaml
# Prometheus alert rules
groups:
- name: mandate
  rules:
  - alert: VaultDown
    expr: up{job="mandate"} == 0
    for: 1m
  - alert: SignerBackendOffline
    expr: mandate_signer_backend_status{status="offline"} == 1
  - alert: AttestationDrift
    expr: increase(mandate_attestation_drift_total[5m]) > 0
  - alert: AuditChainBroken
    expr: mandate_audit_chain_integrity_status != 1
  - alert: HighRejectRate
    expr: rate(mandate_payment_requests_total{decision="rejected"}[5m]) > 0.5
  - alert: EmergencyFrozen
    expr: mandate_emergency_state{state="frozen"} == 1
```

### §14.2 Push alerts (independent of vault)

Setup ntfy alebo similar **on a separate host** (nie na rovnakom serveri ako vault — single point of failure).

---

## §15 Common gotchas (compiled list)

- **Disk full (audit log) → vault halts.** Set `space_left_action = HALT` in `auditd.conf` for the mandate audit (per `19_knowledge_base.md §9`). Monitor disk usage.
- **Time skew** → x402 challenges expire silently. Run `chronyc tracking` to verify drift < 1s.
- **HSM USB reconnect** sometimes requires re-enroll. `mandate signer reconnect <key_id>` recovers.
- **AppArmor `complain` mode** during dev; switch to `enforce` for production: `sudo aa-enforce /etc/apparmor.d/usr.bin.mandate`.
- **PCR mismatch after kernel upgrade** → LUKS won't unlock. Have offline passphrase backup. Re-enroll TPM after kernel upgrade with `systemd-cryptenroll --tpm2-pcrs=7+11+14 --wipe-slot=tpm2`.
- **systemd-analyze security score >5** → review which directives are loose; usually `DeviceAllow` pattern too broad.
- **journald disk pressure** → set `SystemMaxUse=1G` in `/etc/systemd/journald.conf` to prevent runaway logs.

---

## §16 Profil-specific install matrix

| Profil | Hardware | LUKS+TPM | HSM | TEE | Setup time |
|---|---|---|---|---|---|
| DEV / Hackathon | any | optional | optional | no | 30 min |
| PRODUCTION-HSM | mini-PC + HSM | yes | **required** | no | 2 hours |
| PRODUCTION-TEE | TDX/SEV server + HSM | yes | recommended | **required** | 4-6 hours |
