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
use serde::{Deserialize, Serialize};
use crate::prelude::*;
use afbv4::prelude::*;
use energy::prelude::*;

pub(crate) fn to_static_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

AfbDataConverter!(api_actions, ApiAction);
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase", tag = "action")]
pub enum ApiAction {
    #[default]
    READ,
    SUBSCRIBE,
    UNSUBSCRIBE,
    INFO,
}

pub struct BindingCfg {
    pub uid: &'static str,
    pub linky_api: &'static str,
    pub meter_api: &'static str,
    pub power_api: &'static str,
    pub energy_mgr: &'static ManagerHandle,
}


struct ApiUserData {
    linky_api: &'static str,
    energy_mgr: &'static ManagerHandle,
}
impl AfbApiControls for ApiUserData {
    // the API is created and ready. At this level user may subcall api(s) declare as dependencies
    fn start(&mut self, api: &AfbApi) ->  Result<(),AfbError> {
        afb_log_msg!(Notice, api, "get linky max power api:{}/PCOUP", self.linky_api);
        let response=AfbSubCall::call_sync(api, self.linky_api, "PCOUP", ApiAction::READ)?;
        let data= response.get::<JsoncObj>(0)?.index::<i32>(0)?;
        self.energy_mgr.set_power_subscription(data*1000)?;

        AfbSubCall::call_sync(api, self.linky_api, "ADPS", ApiAction::SUBSCRIBE)?;

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
    api_actions::register()?;
    types_registers()?;

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
        return afb_error!("energy-binding-config", "mandatory 'linky_api' not defined in binding json config")
    };

    let power_api = if let Ok(value) = jconf.get::<String>("power_api") {
        to_static_str(value)
    } else {
        return afb_error!("energy-binding-config", "mandatory 'power_api' not defined in binding json config")
    };

    let meter_api = if let Ok(value) = jconf.get::<String>("meter_api") {
        to_static_str(value)
    } else {
        return afb_error!("energy-binding-config", "mandatory 'meter_api' not defined in binding json config")
    };

    let permission = if let Ok(value) = jconf.get::<String>("permission") {
        AfbPermission::new(to_static_str(value))
    } else {
        AfbPermission::new("acl:nrj:client")
    };

    // let create the energy manager now in order to share it with verbs/events
    let energy_mgr= ManagerHandle::new(rootv4, power_api);

    let config = BindingCfg {
        uid,
        meter_api,
        linky_api,
        power_api,
        energy_mgr,
    };

    // create backend API
    let api = AfbApi::new(api).set_info(info).set_permission(permission)
            .set_callback(Box::new(ApiUserData {
            linky_api,
            energy_mgr,
        }));

    register_verbs(api, config)?;

    // register api dependencies
    api.require_api(meter_api);
    api.require_api(linky_api);
    //api.require_api(power_api);

    Ok(api.finalize()?)
}

// register binding within libafb
AfbBindingRegister!(binding_init);
