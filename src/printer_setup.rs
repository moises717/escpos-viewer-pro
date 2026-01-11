#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
pub fn install_printer() -> Result<(), String> {
    // Requiere privilegios de admin para crear puertos/impresoras.
    // Usamos cmdlets de PrintManagement: Add-PrinterPort / Add-Printer.
    let script = r#"
$ErrorActionPreference = 'Stop'
$portName = 'ESCPosViewer9100'
$printerName = 'ESCPos Viewer (TCP 9100)'
$driverName = 'Generic / Text Only'

if (-not (Get-PrinterDriver -Name $driverName -ErrorAction SilentlyContinue)) {
  throw \"No se encontró el driver '$driverName'. (Normalmente viene con Windows)\"
}

if (-not (Get-PrinterPort -Name $portName -ErrorAction SilentlyContinue)) {
  try {
    Add-PrinterPort -Name $portName -PrinterHostAddress '127.0.0.1' -PortNumber 9100 -SNMPEnabled $false | Out-Null
  } catch {
    # Algunas versiones no soportan -SNMPEnabled
    Add-PrinterPort -Name $portName -PrinterHostAddress '127.0.0.1' -PortNumber 9100 | Out-Null
  }
}

if (-not (Get-Printer -Name $printerName -ErrorAction SilentlyContinue)) {
  Add-Printer -Name $printerName -DriverName $driverName -PortName $portName | Out-Null
}

Write-Output \"OK: $printerName\"
"#;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| format!("No se pudo ejecutar PowerShell: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "Falló instalación de impresora.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        ))
    }
}

#[cfg(windows)]
pub fn uninstall_printer() -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$portName = 'ESCPosViewer9100'
$printerName = 'ESCPos Viewer (TCP 9100)'

Get-Printer -Name $printerName | Remove-Printer | Out-Null
Get-PrinterPort -Name $portName | Remove-PrinterPort | Out-Null

Write-Output \"OK: removed (if existed)\"
"#;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| format!("No se pudo ejecutar PowerShell: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "Falló desinstalación de impresora.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        ))
    }
}

#[cfg(not(windows))]
pub fn install_printer() -> Result<(), String> {
    Err("Instalación de impresora solo soportada en Windows".to_string())
}

#[cfg(not(windows))]
pub fn uninstall_printer() -> Result<(), String> {
    Err("Desinstalación de impresora solo soportada en Windows".to_string())
}
