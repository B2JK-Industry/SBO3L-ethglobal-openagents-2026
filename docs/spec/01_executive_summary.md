# A. Executive Summary

**SBO3L** je lokálny bezpečnostný produkt pre vlastný Linux server, ktorý funguje ako *platobný koprocesor* a policy firewall pre AI agentov. Technický daemon/protokol v dokumentácii zatiaľ používa namespace `mandate`.

Rieši rastúci a zatiaľ neuspokojený problém: autonómni AI agenti čoraz častejšie potrebujú vykonávať mikroplatby — za API volania (x402), za model inference, za dáta, za prácu iných agentov, za výpočet — ale nikto im nemôže zveriť private key. Agent runtime je príliš veľká útočná plocha (prompt injection, kompromitované toolingy, supply chain útoky), a klasická hardvérová peňaženka je naopak navrhnutá tak, že každú transakciu musí potvrdiť človek tlačidlom — čo robí Trezor a Ledger pre autonómne mikroplatby nepoužiteľné.

Produkt je určený pre **vývojárov agentických systémov, výskumné tímy, produktové firmy stavajúce machine-to-machine ekonomiku, a pre odvážnych jednotlivcov**, ktorí chcú prevádzkovať agentov doma alebo na vlastnom serveri bez toho, aby museli zveriť kľúče tretej strane (Coinbase Agentic Wallets, Turnkey, Privy a podobné custodial riešenia).

Klasický **Trezor nestačí**, lebo vyžaduje fyzické potvrdenie a nemá programovateľnú policy vrstvu pre stovky mikroplatieb denne. **Server wallet (geth keystore, ethers signer, web3.js)** zase nestačí, lebo private key žije v rovnakom procese a pamäti ako kompromitovateľný agent — jediný úspešný RCE alebo memory dump znamená stratu treasury.

Unikátnosť produktu je v tom, že **rozdeľuje rozhodovanie a podpis na dve trust domény**: žiadosť agenta prejde cez podpísanú, versionovanú policy-as-code, prípadne cez transaction simulator a x402 verifier, a iba schválený, normalizovaný payload sa pošle do izolovaného signera (TPM, HSM, secure element, alebo TEE-sealed key). Každé rozhodnutie je hash-chained a audit log je podpísaný. Cieľová verzia beží v TEE (Intel TDX / AMD SEV-SNP) a vie poskytnúť **remote attestation** — dôkaz pre tretiu stranu (alebo on-chain smart account), že podpis vznikol cez správny vault runtime nad správnymi pravidlami.

Hlavné komponenty: Agent Payment Request Protocol, Policy Engine (Rego/CEL), Budget Ledger, x402 Verifier, Transaction Simulator, Signing Adapter Layer (TPM/HSM/PKCS#11/MPC), Attestation Layer, Signed Audit Log, Human Approval Gateway a Emergency Controls (kill switch, freeze, rotate).

**MVP** je linuxový daemon (`mandate`) s lokálnym REST/gRPC API, YAML policy, encrypted file key, x402 verifierom pre Base/USDC, budget ledgerom v SQLite a CLI/web UI pre admin. Verejný názov produktu je **SBO3L**: agent nedostáva wallet, dostáva spending mandate. **Dlhodobá vízia** je open-source referenčná implementácia + appliance distribúcia (NUC/mini-PC image) s TEE-backed runtime, HSM podporou a integráciou na ERC-4337 smart accounty cez session keys, kde on-chain kontrakt verifikuje TEE attestation pri každom podpise.
