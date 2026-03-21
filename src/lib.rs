pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_default_skeleton.rs"));
    #[cfg(feature = "animations")]
    include!(concat!(env!("OUT_DIR"), "/Stand.rs"));
}
