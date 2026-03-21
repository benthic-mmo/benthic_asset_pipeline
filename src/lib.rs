pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_default_skeleton.rs"));
}

// TODO: rewrite this to use the default animations hashmap provided by the other repo
// make this a macro that concats all of these includes
#[cfg(feature = "animations")]
pub mod animations {
    pub mod stand {
        include!(concat!(env!("OUT_DIR"), "/Stand.rs"));
    }
    pub mod bow {
        include!(concat!(env!("OUT_DIR"), "/Bow.rs"));
    }
}
