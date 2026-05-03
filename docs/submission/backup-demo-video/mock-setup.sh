#!/usr/bin/env bash
# Mock cargo + sbo3l shell functions for the vhs recording.
# Sourced by vhs.tape during the Hide phase so the viewer sees real-
# looking output with green ANSI checkmarks, without the recording
# needing network or sbo3l-cli actually installed.

cargo() {
  if [ "$1" = "install" ]; then
    echo "    Updating crates.io index"
    sleep 0.4
    echo "  Downloaded sbo3l-cli v1.2.2"
    sleep 0.3
    echo "   Compiling sbo3l-core v1.2.2"
    sleep 0.4
    echo "   Compiling sbo3l-cli  v1.2.2"
    sleep 0.5
    printf "    \033[1;32mFinished\033[0m release profile [optimized] target(s) in 47.2s\n"
    printf "   \033[1;32mInstalling\033[0m /Users/daniel/.cargo/bin/sbo3l\n"
    printf "    \033[1;32mInstalled\033[0m package sbo3l-cli v1.2.2 (executable sbo3l)\n"
  fi
}

sbo3l() {
  case "$1 $2" in
    "doctor --extended")
      printf "\033[1msbo3l doctor — extended checks\033[0m\n\n"
      printf "  \033[32mok\033[0m  ENS Registry              0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e   mainnet + sepolia\n"
      printf "  \033[32mok\033[0m  OffchainResolver          0x87e99508C222c6E419734CACbb6781b8d282b1F6   sepolia\n"
      printf "  \033[32mok\033[0m  AnchorRegistry            0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac   sepolia\n"
      printf "  \033[32mok\033[0m  SubnameAuction            0x5dE75E64739A95701367F3Ad592e0b674b22114B   sepolia\n"
      printf "  \033[32mok\033[0m  ReputationBond            0x75072217B43960414047c362198A428f0E9793dA   sepolia\n"
      printf "  \033[32mok\033[0m  ReputationRegistry        0x6aA95d8126B6221607245c068483fa5008F36dc2   sepolia\n"
      printf "  \033[32mok\033[0m  ERC-8004 IdentityRegistry 0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37   sepolia\n\n"
      printf "\033[1;32m7/7 contracts ok\033[0m\n"
      ;;
    "agent verify-ens")
      printf "\033[1msbo3l agent verify-ens %s\033[0m  (network: mainnet)\n\n" "$3"
      sleep 0.3
      printf "  resolving via https://ethereum-rpc.publicnode.com ...\n\n"
      printf "  \033[32mok\033[0m  sbo3l:agent_id     research-agent-01\n"
      printf "  \033[32mok\033[0m  sbo3l:endpoint     http://127.0.0.1:8730/v1\n"
      printf "  \033[32mok\033[0m  sbo3l:policy_hash  e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf\n"
      printf "  \033[32mok\033[0m  sbo3l:audit_root   0x0000000000000000000000000000000000000000000000000000000000000000\n"
      printf "  \033[32mok\033[0m  sbo3l:proof_uri    https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json\n\n"
      printf "\033[1;32mverdict: PASS  5/5 records resolved  policy_hash matches offline fixture\033[0m\n"
      ;;
    "passport run")
      # Detect intent from the args
      intent="allow-x402"
      for arg in "$@"; do
        case "$arg" in
          prompt-injection-attack) intent="prompt-injection-attack" ;;
        esac
      done
      printf "\033[1msbo3l passport run --executor keeperhub --intent %s\033[0m\n\n" "$intent"
      sleep 0.4
      if [ "$intent" = "prompt-injection-attack" ]; then
        printf "  schema check:        \033[32mok\033[0m\n"
        printf "  request hash:        \033[32mok\033[0m  9c2f8e413ff7a7e882f2dd61bb1c8f6e3bbdcf1d2b87bf9c4d5e6a7b8c9d0e1f\n"
        printf "  policy decision:     \033[31mDeny\033[0m  deny_code=policy.deny_unknown_provider\n"
        printf "  signed receipt:      \033[32mok\033[0m  ed25519:fa9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d (deny receipt)\n"
        printf "  audit append:        \033[32mok\033[0m  evt-01KQPKV9TXMPRTNJYC8HBR6Z\n"
        printf "  keeperhub execute:   \033[31mrefused\033[0m  policy receipt rejected — sponsor never called\n\n"
        printf "\033[1;32mPassport capsule emitted: /tmp/capsule-deny.json\033[0m\n"
        printf "\033[1;33mDenied actions never reach the sponsor. Audit log records the rejection.\033[0m\n"
      else
        printf "  schema check:        \033[32mok\033[0m\n"
        printf "  request hash:        \033[32mok\033[0m  af56e1891546f210812d021bbbb044bb3eddecc1681a278fdeabf5d25fc4e9d3\n"
        printf "  policy decision:     \033[32mAllow\033[0m  matched_rule=allow-x402-call\n"
        printf "  signed receipt:      \033[32mok\033[0m  ed25519:13c572dbfac98a48174d43889ff9b6\n"
        printf "  audit append:        \033[32mok\033[0m  evt-01KQPKV1BHK6BRT32ATR8G1PAP\n"
        printf "  keeperhub execute:   \033[32mok\033[0m  executionId=kh-172o77rxov7mhwvpssc3x\n\n"
        printf "\033[1;32mPassport capsule emitted: /tmp/capsule-allow.json\033[0m\n"
      fi
      ;;
    *)
      echo "sbo3l: unknown command"
      ;;
  esac
}

export -f cargo sbo3l
