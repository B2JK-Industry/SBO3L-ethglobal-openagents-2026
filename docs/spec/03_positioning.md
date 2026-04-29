# C. Product Positioning

## C.1 Voči existujúcim kategóriám

### vs. Trezor / Ledger
**Oni:** "Hardvérová peňaženka pre človeka, ktorý fyzicky podpisuje každú transakciu."
**My:** "Hardvérovo izolovaný platobný koprocesor pre AI agenta, ktorý rozhoduje podľa programovateľnej, podpísanej a auditovanej policy."

Trezor predpokladá, že potvrdzovateľ je *človek pri zariadení*. SBO3L predpokladá, že potvrdzovateľ je *deterministická policy*, a človek je až eskalačný bod (approval gateway, kill switch).

### vs. MetaMask / browser wallet
**Oni:** "Wallet pre interaktívneho usera v prehliadači."
**My:** "Wallet-less server-side adapter; agent nemá wallet, agent má payment request endpoint."

MetaMask sa stará o user experience podpisu. My sa staráme o to, aby podpis nebol potrebný v hot pathe agenta.

### vs. Coinbase Agentic Wallets / CDP / Turnkey
**Oni:** "Custodial agent wallet ako služba, hostovaná u nás."
**My:** "Self-hosted agent vault na vlastnom Linuxe, bez vendor trustu, s open-source policy enginom a podporou vlastného HSM/TEE."

Custodial služby sú rýchla cesta, ale presúvajú dôveru a regulatórnu zodpovednosť na vendora. Náš produkt je sovereign-first.

### vs. čistý HSM setup (YubiHSM, Nitrokey, CloudHSM)
**Oni:** "HSM podpíše čo mu pošleš."
**My:** "Vault rozhoduje *či* a *čo* sa má podpísať; HSM je iba jedna z možných signing backendov."

HSM je primitivum. My sme produkt nad ním.

### vs. čistý smart account / ERC-4337 setup
**Oni:** "On-chain validator a session keys."
**My:** "Off-chain decision engine + signer pre tieto session keys; doplňujeme on-chain logiku off-chain runtime gateway."

Smart account vie kontrolovať *čo* sa stane on-chain, ale nevie nič o off-chain semantike (x402 challenge validity, provider reputation, simulator output). Sme komplementárni.

### vs. čistý x402 payment flow
**Oni:** "Protokol pre HTTP 402 mikroplatby."
**My:** "Execution environment pre x402 + ďalšie protokoly, s policy a key isolation."

x402 nehovorí *kde* žije kľúč, *kto* rozhoduje, alebo *ako* sa to auditovuje. To je naša rola.

---

## C.2 Mentálny model

| Vrstva | Reprezentácia | Zodpovednosť |
|---|---|---|
| Trezor | Hardvérový trezor pre človeka | Drží kľúč, čaká na klik |
| Browser wallet | Pohodlný interaktívny signer | UX podpisu |
| Custodial wallet | Platforma | Outsourcing dôvery |
| HSM | Trezor na kľúče | Drží kľúč, podpisuje payload |
| TEE | Izolovaný runtime | Bezpečné vykonávanie kódu |
| Smart account | On-chain policy vrstva | On-chain validácia |
| x402 | Payment protokol | Štandardizácia mikroplatby |
| **SBO3L** | **Lokálny agent mandate/payment control plane** | **Rozhoduje, podpisuje, auditovuje, attestuje** |

SBO3L je *kontrolná rovina* (control plane) pre agentické platby a on-chain akcie. Stretáva sa všetkým — HSM používa ako backend, TEE ako runtime, smart account ako settlement layer, x402 ako protocol — ale je samostatný, doménovo zameraný produkt. Interný daemon/protokol môže v kóde stále niesť namespace `mandate`.

---

## C.3 Tri positioning vety

### 1. Technická veta (security architekt)
> "SBO3L je policy-driven, attestable, lokálne hostovaný platobný koprocesor pre AI agentov, ktorý oddeľuje rozhodnutie od podpisu cez podpísanú policy-as-code a HSM/TEE-backed signer, takže žiadny kompromitovaný agent runtime nedokáže obísť spending limity ani exfiltrovať private key."

### 2. Investor veta (VC / strategic)
> "Postavili sme SBO3L: open-source spending mandate layer pre AI agentov, ktorý umožňuje vývojárom a firmám prevádzkovať autonómnych agentov s reálnymi peniazmi na vlastnom hardware bez toho, aby museli zveriť kľúče Coinbase alebo Turnkey — odomykáme sovereign agentic economy."

### 3. Developer veta (engineer / DevRel)
> "Daj svojmu agentovi mandát cez `POST /v1/payment-requests` a SBO3L sa postará o policy, x402, simulation, signing a audit. Agent nikdy neuvidí private key. Beží lokálne, deployuje sa cez `docker compose up` alebo cez appliance image, a vie podpisovať cez tvoj YubiHSM, TPM, alebo SGX/TDX enclave."

---

## C.4 Anti-positioning (čím produkt nie je)

- **Nie je to ďalší MetaMask.** Nemá UI peňaženky pre človeka.
- **Nie je to ďalší Coinbase Agent Wallet.** Nie je custodial, nie je hostovaný.
- **Nie je to ďalší smart contract wallet.** Beží off-chain, nie on-chain.
- **Nie je to ďalší MPC vendor.** MPC môže byť backend, nie celý produkt.
- **Nie je to ďalší L2 alebo chain.** Je chain-agnostický, primárne EVM, ale rozšíriteľný.
- **Nie je to univerzálny KMS.** Je doménovo zameraný na agent payments.
