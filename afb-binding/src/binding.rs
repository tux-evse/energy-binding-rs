/*
 * Copyright (C) 2015-2022 IoT.bzh Company
 * Author: Fulup Ar Foll <fulup@iot.bzh>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 */
use crate::prelude::*;
use afbv4::prelude::*;
use energy::prelude::*;
use typesv4::prelude::*;

pub struct BindingCfg {
    pub linky_api: &'static str,
    pub meter_api: &'static str,
    pub energy_mgr: &'static ManagerHandle,
    pub tic: u32,
}

struct ApiUserData {
    linky_api: &'static str,
    energy_mgr: &'static ManagerHandle,
}
impl AfbApiControls for ApiUserData {
    // the API is created and ready. At this level user may subcall api(s) declare as dependencies
    fn start(&mut self, api: &AfbApi) -> Result<(), AfbError> {
        // if linky_api defined subscribe to over current notification
        if self.linky_api != "" {
            afb_log_msg!(
                Notice,
                api,
                "get linky max power api:{}/PCOUP",
                self.linky_api
            );

            let response = AfbSubCall::call_sync(api, self.linky_api, "PCOUP", EnergyAction::READ)?;
            let max_power = response.get::<JsoncObj>(0)?.index::<i32>(0)?;
            let response = AfbSubCall::call_sync(api, self.linky_api, "URMS", EnergyAction::READ)?;
            let cur_tension = response.get::<JsoncObj>(0)?.index::<i32>(0)?;
            self.energy_mgr
                .set_power_subscription(max_power * 1000, cur_tension)?;

            AfbSubCall::call_sync(api, self.linky_api, "ADPS", EnergyAction::SUBSCRIBE)?;
        }
        Ok(())
    }

    // mandatory unsed declaration
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

// Binding init callback started at binding load time before any API exist
// -----------------------------------------
pub fn binding_init(rootv4: AfbApiV4, jconf: JsoncObj) -> Result<&'static AfbApi, AfbError> {
    afb_log_msg!(Info, rootv4, "config:{}", jconf);

    // add binding custom converter
    engy_registers()?;

    let uid = jconf.default::<&'static str>("uid", "energy-mgr")?;
    let api = jconf.default::<&'static str>("api", uid)?;
    let info = jconf.default::<&'static str>("info", "")?;

    let imax = jconf.default::<i32>("imax", 32)?;
    let pmax = jconf.default::<i32>("pmax", 22)?;
    let umax = jconf.default::<i32>("umax", 245)?;
    let phase = jconf.default::<i32>("phase", 3)?;

    let linky_api = jconf.default::<&'static str>("linky_api", "")?;
    let meter_api = jconf.default::<&'static str>("meter_api", "modbus")?;

    // Create the energy manager now in order to share session authorization it with verbs/events
    let energy_event = AfbEvent::new("over-limit");
    let energy_mgr = ManagerHandle::new(energy_event, imax, pmax, umax, phase);
    let tic = jconf.get::<u32>("tic")?;

    // create backend API
    let api = AfbApi::new(api)
        .set_info(info)
        .add_event(energy_event)
        .add_event(energy_event)
        .set_callback(Box::new(ApiUserData {
            linky_api,
            energy_mgr,
        }));

    let config = BindingCfg {
        meter_api,
        linky_api,
        energy_mgr,
        tic,
    };

    // register api dependencies
    api.require_api(meter_api);
    if linky_api != "" {
        api.require_api(linky_api);
    }
    if let Ok(value) = jconf.get::<String>("permission") {
        api.set_permission(AfbPermission::new(to_static_str(value)));
    };

    register_verbs(api, config)?;

    Ok(api.finalize()?)
}

// register binding within libafb
AfbBindingRegister!(binding_init);
