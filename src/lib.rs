#[cfg(feature = "animations")]
use std::path::PathBuf;

#[cfg(feature = "animations")]
use strum_macros::Display;

#[cfg(feature = "animations")]
use strum_macros::EnumString;

#[cfg(feature = "animations")]
use uuid::Uuid;

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/default_skeleton.rs"));
}

macro_rules! define_animations {
    (
        $( $name:ident => $uuid:expr ),* $(,)?
    ) => {

        pub mod animations {
            $(
                pub mod $name {
                    include!(concat!(
                        env!("OUT_DIR"),
                        "/",
                        stringify!($name),
                        ".rs"
                    ));
                }
            )*
        }
        pub fn joints(&self) -> &'static [Joint] {
            match self {
                $(
                    Self::$name => &crate::generated::animations::$name::JOINTS,
                )*
            }
        }
    };
}
