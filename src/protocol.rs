pub const PIXELS_PER_LINE: usize = 384;
const CCITT: crc::Crc<u8> = crc::Crc::<u8>::new(&crc::CRC_8_SMBUS);

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Feed(FeedDirection, u8),
    Print(bool, usize, [u8; PIXELS_PER_LINE / 8]), // One bit per pixel in uncompressed mode
    GetDeviceStatus,
    SetQuality(Quality),
    MagicLattice(LatticeType),
    GetDeviceInfo,
    UpdateDevice, //???
    SetWifi,      //????
    FlowControl(Flow),
    SetEnergy(u16),
    DeviceId(u8), //????
    SetSpeed(u8),
    SetDrawingMode(DrawingMode),
}

#[derive(Clone, Copy, Debug)]
pub enum Flow {
    Start = 0x00,
    Stop = 0x10,
}

#[derive(Clone, Copy, Debug)]
pub enum LatticeType {
    Start,
    End,
}

#[derive(Clone, Copy, Debug)]
pub enum FeedDirection {
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug)]
pub enum DrawingMode {
    Image = 0x0,
    Text = 0x1,
}

#[derive(Clone, Copy, Debug)]
pub enum Quality {
    Quality1 = 0x31,
    Quality2 = 0x32,
    Quality3 = 0x33,
    Quality4 = 0x34,
    Quality5 = 0x35,
    SpeedThin = 0x22,
    SpeedModeration = 0x23,
    SpeedThick = 0x25,
}

impl Command {
    fn opcode(&self) -> u8 {
        use Command::*;
        match self {
            Feed(FeedDirection::Reverse, _) => 0xA0,
            Feed(FeedDirection::Forward, _) => 0xA1,
            Print(false, _, _) => 0xA2, // uncompressed
            GetDeviceStatus => 0xA3,
            SetQuality(_) => 0xA4,
            MagicLattice(_) => 0xA6,
            GetDeviceInfo => 0xA8,
            UpdateDevice => 0xA9,
            SetWifi => 0xAA,
            FlowControl(_) => 0xAE,
            SetEnergy(_) => 0xAF,
            DeviceId(_) => 0xBB,
            SetSpeed(_) => 0xBD,
            SetDrawingMode(_) => 0xBE,
            Print(true, _, _) => 0xBf, // compressed
        }
    }

    fn payload(&self) -> Vec<u8> {
        use Command::*;
        match self {
            Feed(_, len) => vec![*len, 0],
            Print(_, len, data) => data[0..*len].to_vec(),
            GetDeviceStatus => vec![0],
            SetQuality(quality) => vec![*quality as u8],
            MagicLattice(LatticeType::Start) => vec![
                0xAA, 0x55, 0x17, 0x38, 0x44, 0x5F, 0x5F, 0x5F, 0x44, 0x38, 0x2C,
            ],
            MagicLattice(LatticeType::End) => vec![
                0xAA, 0x55, 0x17, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17,
            ],
            GetDeviceInfo => vec![0],
            UpdateDevice => vec![0], // ????
            FlowControl(flow) => vec![*flow as u8],
            SetEnergy(energy) => vec![(energy & 0xFF) as u8, ((energy >> 8) & 0xFF) as u8],
            DeviceId(id) => vec![*id], // ?????
            SetSpeed(speed) => vec![*speed],
            SetDrawingMode(mode) => vec![*mode as u8],

            _ => vec![],
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let opcode = self.opcode();
        let payload = self.payload();

        let mut crc = CCITT.digest();
        crc.update(&payload);
        let crc = crc.finalize() as u8;

        let payload_len = payload.len();

        let mut bytes = vec![
            0x51,
            0x78,
            opcode,
            0x00, /* Sent by host */
            payload_len as u8,
            0x00,
        ];

        bytes.extend(payload);

        bytes.extend(vec![crc, 0xFF]);

        bytes
    }
}
