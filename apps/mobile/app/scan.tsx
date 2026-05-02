import { CameraView, useCameraPermissions } from "expo-camera";
import { useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { useRouter } from "expo-router";
import { tokens } from "~/theme";

// Capsule QR scanner. The QR encodes a base64-url Passport capsule
// payload. We hand it off to the verify route which mounts the WASM
// verifier (same code path as the hosted-app /demo/3-verify-yourself).
//
// Self-review fix (bug 5): switched from expo-barcode-scanner (removed
// in Expo SDK 52) to expo-camera's built-in barcode scanning.
// CameraView accepts barcodeScannerSettings directly; no separate
// package needed.

export default function ScanScreen(): JSX.Element {
  const [permission, requestPermission] = useCameraPermissions();
  const [scanned, setScanned] = useState(false);
  const router = useRouter();

  if (!permission) return <View style={styles.center}><Text style={styles.muted}>Loading…</Text></View>;
  if (!permission.granted) {
    return (
      <View style={styles.center}>
        <Text style={styles.error}>Camera permission required to scan capsules.</Text>
        <TouchableOpacity onPress={requestPermission} style={styles.btn}>
          <Text style={styles.btnText}>Grant permission</Text>
        </TouchableOpacity>
      </View>
    );
  }

  const onScan = ({ data }: { data: string }): void => {
    if (scanned) return;
    setScanned(true);
    router.push({ pathname: "/verify", params: { capsule: data } });
  };

  return (
    <View style={styles.container}>
      <CameraView
        style={StyleSheet.absoluteFillObject}
        facing="back"
        barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
        onBarcodeScanned={scanned ? undefined : onScan}
      />
      <View style={styles.frame}><Text style={styles.frameText}>Align Passport QR inside the frame</Text></View>
      {scanned && (
        <TouchableOpacity onPress={() => setScanned(false)} style={styles.retry}>
          <Text style={styles.retryText}>Tap to scan again</Text>
        </TouchableOpacity>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: tokens.bg },
  center: { flex: 1, justifyContent: "center", alignItems: "center", padding: 16, backgroundColor: tokens.bg },
  muted: { color: tokens.muted },
  error: { color: tokens.deny, marginBottom: 16 },
  btn: { backgroundColor: tokens.accent, paddingVertical: 12, paddingHorizontal: 24, borderRadius: tokens.rMd },
  btnText: { color: tokens.bg, fontSize: 15, fontWeight: "700" },
  frame: { position: "absolute", left: 40, right: 40, top: "30%", height: 280, borderColor: tokens.accent, borderWidth: 2, borderRadius: tokens.rLg, justifyContent: "flex-end", padding: 8 },
  frameText: { color: tokens.accent, fontSize: 12, textAlign: "center" },
  retry: { position: "absolute", bottom: 40, alignSelf: "center", padding: 14, backgroundColor: tokens.codeBg, borderRadius: tokens.rMd },
  retryText: { color: tokens.fg, fontSize: 14 },
});
