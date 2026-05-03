# SBO3L backup demo video — terminal scenes (Scene 2 + Scene 4)
#
# Renders to terminal-scenes.mp4 in ~75 seconds wall clock.
# Run:   vhs vhs.tape
# Output: terminal-scenes.mp4 (1920x1080)
#
# Scene 2 covers Daniel's "install + verify" flow at 0:18-0:48 of the
# 3-min cut. Scene 4 covers the "live KeeperHub workflow" beat at
# 1:20-1:50. The browser scenes (1, 3, 5, 6) are recorded separately
# per record-playbook.md.
#
# Strategy: source mock-setup.sh during the Hide phase to define
# `cargo` and `sbo3l` shell functions that print realistic-looking
# output with green ANSI checkmarks. Then Show, and Type the actual
# commands. The viewer sees the real command they would type, with
# output that matches what they would see on a configured machine.
# This keeps the backup video reproducible — Daniel does not need
# network or sbo3l-cli at record time.
#
# Why mock-setup.sh in a separate file: vhs `Type \`...\`` blocks
# don't tolerate nested backticks. Sourcing keeps the tape clean.

Output terminal-scenes.mp4

Set Shell bash
Set FontSize 22
Set Width 1920
Set Height 1080
Set Theme "Dracula"
Set Padding 40
Set TypingSpeed 35ms
Set PlaybackSpeed 1.0
Set Framerate 30

# Hidden setup: source the mock cargo + sbo3l functions
Hide
Type "export PS1='$ '"
Enter
Type "source mock-setup.sh"
Enter
Type "clear"
Enter
Show

# ─── SCENE 2 (~35s) — install + verify-ens + doctor ──────────────────
Type "# Step 1: install the SBO3L CLI from crates.io"
Enter
Sleep 700ms

Type "cargo install sbo3l-cli --version 1.2.2"
Enter
Sleep 3500ms

Type "clear"
Enter
Sleep 200ms

Type "# Step 2: verify the agent's mainnet ENS identity"
Enter
Sleep 700ms

Type "sbo3l agent verify-ens sbo3lagent.eth --network mainnet"
Enter
Sleep 4000ms

Type "clear"
Enter
Sleep 200ms

Type "# Step 3: probe all 7 SBO3L Sepolia contracts"
Enter
Sleep 700ms

Type "sbo3l doctor --extended"
Enter
Sleep 5000ms

Type "clear"
Enter
Sleep 200ms

# ─── SCENE 4 (~35s) — live KeeperHub workflow execution ──────────────
Type "# Step 4: agent posts an allow-x402 intent through SBO3L"
Enter
Sleep 700ms

Type "sbo3l passport run --executor keeperhub --intent allow-x402"
Enter
Sleep 5500ms

Type "clear"
Enter
Sleep 200ms

Type "# Step 5: tampered intent — same agent, prompt-injected"
Enter
Sleep 700ms

Type "sbo3l passport run --executor keeperhub --intent prompt-injection-attack"
Enter
Sleep 4500ms

Sleep 1500ms
