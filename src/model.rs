#[derive(Clone, Debug)]
pub enum CommandType {
    Text(String),
    Control(Control),
    Unknown(u8),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Control {
    Newline,
    Tab, // HT (0x09) - Horizontal Tab
    Init,
    Bold(bool),
    Align(Align),
    /// Cambio de tabla de caracteres (ESC t n) interpretado a CodePage.
    CodePage(CodePage),
    /// Raw size byte as received by GS ! n.
    Size {
        raw: u8,
        width: u8,
        height: u8,
    },
    Cut,

    /// Raster bit image: GS v 0
    /// width_bytes = bytes por fila (ancho en bits = width_bytes * 8)
    RasterImage {
        m: u8,
        width_bytes: u16,
        height: u16,
        data: Vec<u8>,
    },

    /// QR generado con comandos GS ( k (Model/Size/ECC/Store/Print)
    Qr {
        model: u8,
        module_size: u8,
        ecc: u8,
        data: Vec<u8>,
    },

    /// Barcode: GS k
    Barcode {
        m: u8,
        data: Vec<u8>,
    },

    /// Configuración de barcode (HRI/alto/ancho/fuente).
    BarcodeHriPosition(BarcodeHriPosition),
    BarcodeHeight(u8),
    BarcodeModuleWidth(u8),
    BarcodeHriFont(u8),

    EscUnknown(u8),
    GsUnknown(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarcodeHriPosition {
    None,
    Above,
    Below,
    Both,
}

#[derive(Clone, Debug)]
pub struct PrinterState {
    pub is_bold: bool,
    pub alignment: Align,
    pub font_scale: f32,
    pub char_width_mul: u8,
    pub char_height_mul: u8,

    pub barcode_hri: BarcodeHriPosition,
    pub barcode_height: u8,
    pub barcode_module_width: u8,
    pub barcode_hri_font: u8,
}

impl Default for PrinterState {
    fn default() -> Self {
        Self {
            is_bold: false,
            alignment: Align::Left,
            font_scale: 1.0,
            char_width_mul: 1,
            char_height_mul: 1,

            barcode_hri: BarcodeHriPosition::None,
            // Valores típicos (pueden variar por impresora, pero sirven para preview).
            barcode_height: 80,
            barcode_module_width: 3,
            barcode_hri_font: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaperWidth {
    W58mm,
    W80mm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodePage {
    Utf8Lossy,
    Cp437,
    Cp850,
    Windows1252,
}
