use std::{collections::HashMap, fmt::Display};

use gamegirl::{
    common::common::input::Button::*,
    frontend::input::{
        InputAction::{self, *},
        InputSource, Key,
    },
};
use gtk::gdk;
use serde::{
    Deserialize, Serialize,
    de::{self, Visitor},
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct GtkKey(gdk::Key);

impl From<gdk::Key> for GtkKey {
    fn from(value: gdk::Key) -> Self {
        Self(value)
    }
}

impl Display for GtkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name().unwrap())
    }
}

impl Key for GtkKey {
    fn is_escape(self) -> bool {
        self == gdk::Key::Escape.into()
    }

    fn default_map() -> HashMap<InputSource<Self>, InputAction> {
        HashMap::from([
            (InputSource::Key(Self(gdk::Key::X)), Button(A)),
            (InputSource::Key(Self(gdk::Key::Z)), Button(B)),
            (InputSource::Key(Self(gdk::Key::Return)), Button(Start)),
            (InputSource::Key(Self(gdk::Key::space)), Button(Select)),
            (InputSource::Key(Self(gdk::Key::Down)), Button(Down)),
            (InputSource::Key(Self(gdk::Key::Up)), Button(Up)),
            (InputSource::Key(Self(gdk::Key::Left)), Button(Left)),
            (InputSource::Key(Self(gdk::Key::Right)), Button(Right)),
            (InputSource::Key(Self(gdk::Key::A)), Button(L)),
            (InputSource::Key(Self(gdk::Key::S)), Button(R)),
            (InputSource::Key(Self(gdk::Key::R)), Hotkey(4)),
        ])
    }
}

impl Serialize for GtkKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.0.name().unwrap())
    }
}

impl<'de> Deserialize<'de> for GtkKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrVisitor;
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = GtkKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("String repr of key")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GtkKey(
                    gdk::Key::from_name(v).ok_or(de::Error::custom("Invalid key"))?,
                ))
            }
        }
        deserializer.deserialize_string(StrVisitor)
    }
}
