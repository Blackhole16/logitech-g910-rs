use keys::{StandardKey, GamingKey, KeyType};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyColor {
    key_code: u8,
    color: Color,
}

#[repr(C, packed)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorPacket {
    head1: u8,
    head2: u8,
    head3: u8,
    head4: u8,
    key_type0: u8,
    pub key_type: KeyType,
    reserved: u8,
    len: u8,
    colors: [KeyColor; 14],
}

#[repr(C, packed)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushPacket {
    head1: u8,
    head2: u8,
    head3: u8,
    head4: u8,
    zero1: u64,
    zero2: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyColorError {
    PacketFull,
    InvalidKeyType,
}

impl Color {
    pub fn new(red: u8, green: u8, blue: u8) -> Color {
        Color {
            red: red,
            green: green,
            blue: blue,
        }
    }
}

impl KeyColor {
    pub fn new(key_code: u8, color: Color) -> KeyColor {
        KeyColor {
            key_code: key_code,
            color: color,
        }
    }
    pub fn new_standard(key_code: StandardKey, color: Color) -> KeyColor {
        KeyColor::new(key_code as u8, color)
    }
    pub fn new_gaming(key_code: GamingKey, color: Color) -> KeyColor {
        KeyColor::new(key_code as u8, color)
    }
}

impl ColorPacket {
    pub fn new(key_type: KeyType) -> ColorPacket {
        let color = match key_type {
            KeyType::Standard => KeyColor::new_standard(StandardKey::None, Color::new(0,0,0)),
            KeyType::Gaming => KeyColor::new_gaming(GamingKey::None, Color::new(0,0,0)),
            KeyType::Memory => unimplemented!()
        };
        let colors = [color.clone(), color.clone(), color.clone(), color.clone(),
            color.clone(), color.clone(), color.clone(), color.clone(), color.clone(),
            color.clone(), color.clone(), color.clone(), color.clone(),color];
        ColorPacket {
            head1: 0x12,
            head2: 0xff,
            head3: 0x0f,
            head4: 0x3b,
            key_type0: 0x00,
            key_type: key_type,
            reserved: 0x00,
            len: 0x00,
            colors: colors,
        }
    }

    pub fn new_standard() -> ColorPacket {
        ColorPacket::new(KeyType::Standard)
    }
    pub fn new_gaming() -> ColorPacket {
        ColorPacket::new(KeyType::Gaming)
    }
    pub fn new_memory() -> ColorPacket {
        ColorPacket::new(KeyType::Memory)
    }

    /// Adds a color to this packet
    ///
    /// If this packet is alredy full, Err will be returned.
    /// # Example
    /// ```
    /// let color_packet = ColorPacket::new_standard();
    /// let key_color = KeyColor::new(StandardKey::A, Color::new(64, 128, 255))
    /// assert!(color_packet.add_key_color(key_color) == Ok(()));
    /// assert!(color_packet.color[0..4] == [StandardKey::A as u8, 64, 128, 255])
    /// ```
    pub fn add_key_color(&mut self, key_color: KeyColor) -> Result<(), KeyColorError> {
        // TODO: add check of KeyType
        if self.len as usize >= self.colors.len() {
            Err(KeyColorError::PacketFull)
        } else {
            self.colors[self.len as usize] = key_color;
            self.len += 1;
            Ok(())
        }
    }
}

impl FlushPacket {
    pub fn new() -> FlushPacket {
        FlushPacket {
            head1: 0x11,
            head2: 0xff,
            head3: 0x0f,
            head4: 0x5b,
            zero1: 0x0,
            zero2: 0x0,
        }
    }
}

#[test]
fn test_add_key_color() {
    let mut color_packet = ColorPacket::new_standard();
    assert!(color_packet.len == 0);

    let key_color_a = KeyColor::new(StandardKey::A, Color::new(1, 2, 3));
    assert!(color_packet.add_key_color(key_color_a.clone()) == Ok(()));
    assert!(color_packet.colors[0] == key_color_a);
    assert!(color_packet.len == 1);

    let key_color_b = KeyColor::new(StandardKey::B, Color::new(4, 5, 6));
    assert!(color_packet.add_key_color(key_color_b.clone()) == Ok(()));

    let key_color_c = KeyColor::new(StandardKey::C, Color::new(7, 8, 9));
    for i in 0..12 {
        assert!(color_packet.add_key_color(key_color_c.clone()) == Ok(()));
    }

    assert!(color_packet.colors[0] == key_color_a
            && color_packet.colors[1] == key_color_b);

    for e in color_packet.colors.iter().skip(2) {
        assert!(*e == key_color_c);
    }

    assert!(color_packet.add_key_color(key_color_c.clone()) == Err(()));
}

