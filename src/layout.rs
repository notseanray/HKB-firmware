// voltage increases with actuation
pub const NORTH_DOWN: bool = true;
// trigger threshold
pub const ACTUATION_THRESHOLD: u16 = 10;
// rapid trigger
pub const RAPID_TRIGGER_THRESHOLD: u16 = 5;
// ignore masks when snapping
pub const SNAP_IGNORE_MASK: bool = true;
// save cpu cycles by ignoring any keys below mv
pub const IGNORE_BELOW_MV: u16 = 60;
// time in seconds between default value calibrations
pub const RECALIBRATION_SEC: usize = 10;
// generic USB keyboard vendor ID
pub const USB_VID: u16 = 0x16c0;
// generic USB keyboard product ID
pub const USB_PID: u16 = 0x27db;
// activating key, layer number, default layer is 0
pub const LAYERMASKS: [(usize, usize); 3] = [(19, 1), (66, 2), (67, 3)];
// W/S, A/D, Up/Down, Left/Right
pub const SNAP_PAIRS: [(usize, usize); 4] = [(22, 15), (1, 14), (11, 10), (5, 12)];

