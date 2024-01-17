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

pub(crate) fn to_static_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

pub struct BindingCfg {
    pub uid: &'static str,
    pub linky_api: &'static str,
    pub meter_api: &'static str,
    pub energy_mgr: &'static ManagerHandle,
    pub imax: i32,
    pub pmax: i32,
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
            let data = response.get::<JsoncObj>(0)?.index::<i32>(0)?;
            self.energy_mgr.set_power_subscription(data * 1000)?;

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

    let uid = if let Ok(value) = jconf.get::<String>("uid") {
        to_static_str(value)
    } else {
        "energy"
    };

    let api = if let Ok(value) = jconf.get::<String>("api") {
        to_static_str(value)
    } else {
        uid
    };

    let info = if let Ok(value) = jconf.get::<String>("info") {
        to_static_str(value)
    } else {
        ""
    };


    let imax = if let Ok(value) = jconf.get::<i32>("imax") {
        value
    } else {
        32
    };

    let pmax = if let Ok(value) = jconf.get::<i32>("pmax") {
        value
    } else {
        22
    };


    let linky_api = if let Ok(value) = jconf.get::<String>("linky_api") {
        to_static_str(value)
    } else {
        afb_log_msg!(
            Warning,
            rootv4,
            "optional 'linky_api' not defined in binding json config"
        );
        ""
    };
    let meter_api = to_static_str(jconf.get::<String>("meter_api")?);

    // Create the energy manager now in order to share session authorization it with verbs/events
    let energy_event = AfbEvent::new("energy");
    let energy_mgr = ManagerHandle::new(energy_event, imax, pmax);

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
        uid,
        meter_api,
        linky_api,
        energy_mgr,
        pmax,
        imax,
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
