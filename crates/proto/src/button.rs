use evdevil::event::Key;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u16)]
pub enum Button {
    A = (1 << 0),
    B = (1 << 1),
    Y = (1 << 2),
    X = (1 << 3),
    Start = (1 << 4),
    Select = (1 << 5),
    Up = (1 << 6),
    Down = (1 << 7),
    Left = (1 << 8),
    Right = (1 << 9),
}

impl BitOr for Button {
    type Output = ButtonSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        ButtonSet(self as u16 | rhs as u16)
    }
}

impl BitAnd for Button {
    type Output = ButtonSet;

    fn bitand(self, rhs: Self) -> Self::Output {
        ButtonSet(self as u16 & rhs as u16)
    }
}

impl BitXor for Button {
    type Output = ButtonSet;

    fn bitxor(self, rhs: Self) -> Self::Output {
        ButtonSet(self as u16 ^ rhs as u16)
    }
}

impl From<Button> for Key {
    fn from(button: Button) -> Self {
        match button {
            Button::A => Key::BTN_A,
            Button::B => Key::BTN_B,
            Button::Y => Key::BTN_Y,
            Button::X => Key::BTN_X,
            Button::Start => Key::BTN_START,
            Button::Select => Key::BTN_SELECT,
            Button::Up => Key::BTN_DPAD_UP,
            Button::Down => Key::BTN_DPAD_DOWN,
            Button::Left => Key::BTN_DPAD_LEFT,
            Button::Right => Key::BTN_DPAD_RIGHT,
        }
    }
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Button::A => "A",
            Button::B => "B",
            Button::Y => "Y",
            Button::X => "X",
            Button::Start => "Start",
            Button::Select => "Select",
            Button::Up => "D↑",
            Button::Down => "D↓",
            Button::Left => "D←",
            Button::Right => "D→",
        };
        write!(f, "{label}")
    }
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
// pub struct ButtonMap<V>([Option<V>; 10]); // TODO use variant_count if stabilized

// impl<V> ButtonMap<V> {
//     pub fn new() -> Self
//     where
//         V: Copy,
//     {
//         Self([None; 10])
//     }

//     pub fn insert(&mut self, button: Button, value: V) {
//         self.0[button as usize] = Some(value);
//     }

//     pub fn get(&self, button: Button) -> Option<&V> {
//         self.0[button as usize].as_ref()
//     }
// }

// impl<V> IntoIterator for ButtonMap<V> {
//     type Item = (Button, V);
//     type IntoIter = ButtonMapIter<V>;

//     fn into_iter(self) -> Self::IntoIter {
//         ButtonMapIter {
//             map: self,
//             index: 0,
//         }
//     }
// }

/// Newtype wrapper for the button bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ButtonSet(u16);

impl BitOr for ButtonSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOr<Button> for ButtonSet {
    type Output = Self;

    fn bitor(self, rhs: Button) -> Self::Output {
        Self(self.0 | rhs as u16)
    }
}

impl BitAnd for ButtonSet {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAnd<Button> for ButtonSet {
    type Output = Self;

    fn bitand(self, rhs: Button) -> Self::Output {
        Self(self.0 & rhs as u16)
    }
}

impl BitXor for ButtonSet {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXor<Button> for ButtonSet {
    type Output = Self;

    fn bitxor(self, rhs: Button) -> Self::Output {
        Self(self.0 ^ rhs as u16)
    }
}

impl BitOrAssign for ButtonSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitOrAssign<Button> for ButtonSet {
    fn bitor_assign(&mut self, rhs: Button) {
        self.0 |= rhs as u16;
    }
}

impl BitAndAssign for ButtonSet {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitAndAssign<Button> for ButtonSet {
    fn bitand_assign(&mut self, rhs: Button) {
        self.0 &= rhs as u16;
    }
}

impl BitXorAssign for ButtonSet {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl BitXorAssign<Button> for ButtonSet {
    fn bitxor_assign(&mut self, rhs: Button) {
        self.0 ^= rhs as u16;
    }
}

impl ButtonSet {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(self, button: Button) -> bool {
        self.0 & button as u16 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn difference(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }

    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn into_inner(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ButtonSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for button in self.into_iter() {
            write!(f, " {button}")?;
        }
        write!(f, " }}")
    }
}

impl IntoIterator for ButtonSet {
    type Item = Button;
    type IntoIter = ButtonSetIter;

    fn into_iter(self) -> Self::IntoIter {
        ButtonSetIter {
            set: self,
            index: 0,
        }
    }
}

pub struct ButtonSetIter {
    set: ButtonSet,
    index: u8,
}

impl Iterator for ButtonSetIter {
    type Item = Button;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < 16 {
            let bit = 1 << self.index;
            self.index += 1;

            if self.set.0 & bit != 0 {
                return Some(unsafe { std::mem::transmute(bit) });
            }
        }
        None
    }
}
