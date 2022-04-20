struct Hook {}

impl Hook {
    fn init() {
        udev::MonitorBuilder::new()
            .and_then(|builder| builder.match_subsystem_devtype("power_supply", "BAT"))
            .and_then(|builder| builder.listen())
            .expect("Unable to listen find and listen on battery");

    }
}