#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
pub fn install_printer() -> Result<(), String> {
    // Requires admin privileges to create ports/printers.
    // Uses PrintManagement cmdlets: Add-PrinterPort / Add-Printer.
    // Key fixes for virtual printer status:
    // 1. Try "Microsoft Print to PDF" first - never reports fake errors
    // 2. Disable SNMP and bidirectional support to avoid error flags
    let script = r#"
$ErrorActionPreference = 'Stop'
$portName = 'ESCPosViewer9100'
$printerName = 'ESCPos Viewer (TCP 9100)'

# Try multiple drivers in order of preference
# "Microsoft Print to PDF" - never reports fake errors, works for TCP/IP virtual printers
# "Generic / Text Only" - fallback but may report Error status incorrectly
$drivers = @('Microsoft Print to PDF', 'Generic / Text Only')
$driverName = $null

foreach ($d in $drivers) {
    if (Get-PrinterDriver -Name $d -ErrorAction SilentlyContinue) {
        $driverName = $d
        break
    }
}

if (-not $driverName) {
    throw "No se encontro ningun driver compatible"
}

if (-not (Get-PrinterPort -Name $portName -ErrorAction SilentlyContinue)) {
    # Create TCP/IP port WITHOUT SNMP (SNMP causes error status on virtual printers)
    Add-PrinterPort -Name $portName -PrinterHostAddress 127.0.0.1 -PortNumber 9100 -SNMPEnabled $false | Out-Null
}

if (-not (Get-Printer -Name $printerName -ErrorAction SilentlyContinue)) {
    Add-Printer -Name $printerName -DriverName $driverName -PortName $portName | Out-Null
}

# Disable bidirectional support and SNMP for clean status reporting
$printer = Get-Printer -Name $printerName
if ($printer) {
    Set-Printer -Name $printerName -BiDirectional $false -SNMPCommunity $null
}

Write-Output "OK: $printerName (driver: $driverName)"
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
            "Fallo instalacion de impresora.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
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

Write-Output "OK: removed (if existed)"
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
            "Fallo desinstalacion de impresora.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        ))
    }
}

#[cfg(not(windows))]
pub fn install_printer() -> Result<(), String> {
    Err("Instalacion de impresora solo soportada en Windows".to_string())
}

#[cfg(not(windows))]
pub fn uninstall_printer() -> Result<(), String> {
    Err("Desinstalacion de impresora solo soportada en Windows".to_string())
}