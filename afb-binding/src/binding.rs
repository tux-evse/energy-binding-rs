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
use serde::{Deserialize, Serialize};
use typesv4::prelude::*;

pub(crate) fn to_static_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

AfbDataConverter!(sensor_actions, SensorAction);
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase", tag = "action")]
pub enum SensorAction {
    #[default]
    READ,
    SUBSCRIBE,
    UNSUBSCRIBE,
    RESET,
    INFO,
}

pub struct BindingCfg {
    pub uid: &'static str,
    pub linky_api: &'static str,
    pub meter_api: &'static str,
    pub energy_mgr: &'static ManagerHandle,
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

            let response = AfbSubCall::call_sync(api, self.linky_api, "PCOUP", SensorAction::READ)?;
            let data = response.get::<JsoncObj>(0)?.index::<i32>(0)?;
            self.energy_mgr.set_power_subscription(data * 1000)?;

            AfbSubCall::call_sync(api, self.linky_api, "ADPS", SensorAction::SUBSCRIBE)?;
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
    sensor_actions::register()?;
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

    let meter_api = if let Ok(value) = jconf.get::<String>("meter_api") {
        to_static_str(value)
    } else {
        return afb_error!(
            "energy-binding-config",
            "mandatory 'meter_api' not defined in binding json config"
        );
    };

    let permission = if let Ok(value) = jconf.get::<String>("permission") {
        AfbPermission::new(to_static_str(value))
    } else {
        AfbPermission::new("acl:engy:client")
    };

    // Create the energy manager now in order to share session authorization it with verbs/events
    let authorize_event = AfbEvent::new("authorize");
    let energy_mgr = ManagerHandle::new(authorize_event);

    // create backend API
    let api = AfbApi::new(api)
        .set_info(info)
        .set_permission(permission)
        .add_event(authorize_event)
        .set_callback(Box::new(ApiUserData {
            linky_api,
            energy_mgr,
        }));

    let config = BindingCfg {
        uid,
        meter_api,
        linky_api,
        energy_mgr,
    };

    register_verbs(api, config)?;

    // register api dependencies
    api.require_api(meter_api);
    if linky_api != "" {
        api.require_api(linky_api);
    }

    Ok(api.finalize()?)
}

// register binding within libafb
AfbBindingRegister!(binding_init);
