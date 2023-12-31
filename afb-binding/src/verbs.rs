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
use std::cell::RefCell;
use std::rc::Rc;
use typesv4::prelude::*;

struct LinkyEvtCtx {
    energy_mgr: &'static ManagerHandle,
    data_set: Rc<RefCell<MeterDataSet>>,
    evt: &'static AfbEvent,
}

AfbEventRegister!(LinkyAdpsEvtCtrl, evt_linky_cb, LinkyEvtCtx);
fn evt_linky_cb(_evt: &AfbEventMsg, args: &AfbData, ctx: &mut LinkyEvtCtx) -> Result<(), AfbError> {
    let mut data_set = match ctx.data_set.try_borrow_mut() {
        Err(_) => return afb_error!("energy-LinkyAdps-update", "fail to access energy state"),
        Ok(value) => value,
    };
    let jreply = args.get::<JsoncObj>(0)?;
    for idx in 0..jreply.count()? {
        let value = jreply.index::<f64>(idx)?;
        data_set.update(idx, value)?;
    }
    if data_set.updated {
        ctx.energy_mgr.update_data_set(&data_set)?;
        ctx.evt.broadcast(data_set.clone());
    }
    Ok(())
}

struct AdpsRequestCtx {
    data_set: Rc<RefCell<MeterDataSet>>,
    linky_api: &'static str,
    adps_verb: &'static str,
    evt: &'static AfbEvent,
}
AfbVerbRegister!(LinkyAdpsRequestVerb, adps_request_cb, AdpsRequestCtx);
fn adps_request_cb(
    rqt: &AfbRequest,
    args: &AfbData,
    ctx: &mut AdpsRequestCtx,
) -> Result<(), AfbError> {
    match args.get::<&SensorAction>(0)? {
        SensorAction::READ => {
            let mut data_set = match ctx.data_set.try_borrow_mut() {
                Err(_) => {
                    return afb_error!("energy-LinkyAdps-update", "fail to access energy state")
                }
                Ok(value) => value,
            };

            let response = AfbSubCall::call_sync(
                rqt.get_api(),
                ctx.linky_api,
                ctx.adps_verb,
                SensorAction::READ,
            )?;

            let jreply = response.get::<JsoncObj>(0)?;
            for idx in 0..jreply.count()? {
                let value = jreply.index::<i32>(idx)?;
                match idx {
                    0 => data_set.total = value,
                    1 => data_set.l1 = value,
                    2 => data_set.l2 = value,
                    3 => data_set.l3 = value,
                    _ => return afb_error!("energy-LinkyAdps-update", "invalid index:{}", idx),
                }
            }

            data_set.tag = data_set.tag.clone();
            rqt.reply(data_set.clone(), 0);
        }

        SensorAction::SUBSCRIBE => {
            AfbSubCall::call_sync(
                rqt.get_api(),
                ctx.linky_api,
                ctx.adps_verb,
                SensorAction::SUBSCRIBE,
            )?;
            ctx.evt.subscribe(rqt)?;
            rqt.reply(AFB_NO_DATA, 0);
        }

        SensorAction::UNSUBSCRIBE => {
            ctx.evt.unsubscribe(rqt)?;
            rqt.reply(AFB_NO_DATA, 0);
        }
        _ => {
            return afb_error!(
                rqt.get_uid().as_str(),
                "action not supported use [read|subscribe|unsubscribe]"
            )
        }
    }
    Ok(())
}

struct MeterEvtCtx {
    data_set: Rc<RefCell<MeterDataSet>>,
    labels: &'static [&'static str],
    meter_api: &'static str,
    meter_prefix: &'static str,
    evt: &'static AfbEvent,
    energy_mgr: &'static ManagerHandle,
}

AfbEventRegister!(MeterEvtCtrl, evt_meter_cb, MeterEvtCtx);
fn evt_meter_cb(evt: &AfbEventMsg, args: &AfbData, ctx: &mut MeterEvtCtx) -> Result<(), AfbError> {
    let mut data_set = match ctx.data_set.try_borrow_mut() {
        Err(_) => return afb_error!("energy-metercb-update", "fail to access energy state"),
        Ok(value) => value,
    };

    let value = args.get::<f64>(0)?;

    // move to bytes as rust cannot index str :(
    let full_name = evt.get_name().as_bytes();
    let short_name = match full_name.get(1 + ctx.meter_api.len()..full_name.len()) {
        Some(value) => value,
        None => {
            return afb_error!(
                "energy-metercb-update",
                "evt_meter_cb meter argument not a valid float number"
            )
        }
    };

    for idx in 0..ctx.labels.len() {
        let label = ctx.labels[idx].as_bytes();
        if short_name == label {
            data_set.update(idx, value)?;
            break;
        }
    }

    // to limit the number of events data is updated only when total value is received
    if data_set.updated {
        ctx.energy_mgr.update_data_set(&data_set)?;
        let listeners = ctx.evt.push(data_set.clone());
        // if no one listen then unsubscribe the low level energy meter events
        if listeners <= 0 {
            afb_log_msg!(
                Notice,
                evt,
                "no more listener to energy event({}) rc={}",
                evt.get_uid(),
                listeners
            );
            for label in ctx.labels {
                afb_log_msg!(
                    Notice,
                    evt,
                    "Unsubscribe api:{}/{}",
                    ctx.meter_api,
                    [ctx.meter_prefix, label].join("/")
                );
                AfbSubCall::call_sync(
                    evt.get_apiv4(),
                    ctx.meter_api,
                    [ctx.meter_prefix, label].join("/").as_str(),
                    SensorAction::UNSUBSCRIBE,
                )?;
                afb_log_msg!(
                    Notice,
                    evt,
                    "Unsubscribed api:{}/{}",
                    ctx.meter_api,
                    [ctx.meter_prefix, label].join("/")
                );
            }
        }
    }
    Ok(())
}

struct MeterRequestCtx {
    data_set: Rc<RefCell<MeterDataSet>>,
    meter_api: &'static str,
    meter_prefix: &'static str,
    labels: &'static [&'static str],
    evt: &'static AfbEvent,
}
AfbVerbRegister!(MeterRequestVerb, meter_request_cb, MeterRequestCtx);
fn meter_request_cb(
    rqt: &AfbRequest,
    args: &AfbData,
    ctx: &mut MeterRequestCtx,
) -> Result<(), AfbError> {
    let mut data_set = match ctx.data_set.try_borrow_mut() {
        Err(_) => return afb_error!("energy-meter-update", "fail to access energy state"),
        Ok(value) => value,
    };

    match args.get::<&SensorAction>(0)? {
        SensorAction::READ => {
            for idx in 0..ctx.labels.len() {
                let label = ctx.labels[idx];
                let response = AfbSubCall::call_sync(
                    rqt.get_api(),
                    ctx.meter_api,
                    [ctx.meter_prefix, label].join("/").as_str(),
                    SensorAction::READ,
                )?;
                let data = response.get::<f64>(0)?;
                data_set.update(idx, data)?;

                let value = (data * 100.0).round() as i32;
                match idx {
                    0 => data_set.total = value,
                    1 => data_set.l1 = value,
                    2 => data_set.l2 = value,
                    3 => data_set.l3 = value,
                    _ => return afb_error!("energy-meter-update", "invalid index:{}", idx),
                }
            }
            data_set.tag = data_set.tag.clone();
            rqt.reply(data_set.clone(), 0);
        }

        SensorAction::SUBSCRIBE => {
            afb_log_msg!(Notice, rqt, "Subscribe {}", ctx.evt.get_uid());
            ctx.evt.subscribe(rqt)?;
            for label in ctx.labels {
                AfbSubCall::call_sync(
                    rqt.get_api(),
                    ctx.meter_api,
                    [ctx.meter_prefix, label].join("/").as_str(),
                    SensorAction::SUBSCRIBE,
                )?;
            }
        }

        SensorAction::UNSUBSCRIBE => {
            afb_log_msg!(Notice, rqt, "Unsubscribe {}", ctx.evt.get_uid());
            ctx.evt.unsubscribe(rqt)?;
        }

        // use l1 to provide session power
        SensorAction::RESET => {
            match data_set.tag {
                MeterTagSet::Energy => {}
                _ => return afb_error!(
                    rqt.get_uid().as_str(),
                    "action reset not supported for tag:{:?}",
                    data_set.tag
                )
            }

            // read meeter reset energy counter value
            let response = AfbSubCall::call_sync(
                rqt.get_api(),
                ctx.meter_api,
                [ctx.meter_prefix, ctx.labels[0]].join("/").as_str(),
                SensorAction::READ,
            )?;
            let data = response.get::<f64>(0)?;
            data_set.start = (data * 100.0).round() as i32;
        }
        _ => {
            return afb_error!(
                rqt.get_uid().as_str(),
                "action not supported use [read|subscribe|unsubscribe|reset]"
            )
        }
    }
    rqt.reply(AFB_NO_DATA, 0);
    Ok(())
}

struct ConfRequestCtx {
    energy_mgr: &'static ManagerHandle,
}

AfbVerbRegister!(ConfRequestVerb, conf_request_cb, ConfRequestCtx);
fn conf_request_cb(
    rqt: &AfbRequest,
    args: &AfbData,
    ctx: &mut ConfRequestCtx,
) -> Result<(), AfbError> {
    let jsonc = args.get::<JsoncObj>(0)?;
    afb_log_msg!(Debug, rqt, "update power conf={}", jsonc);

    //automatically subscribe client to energy manager event
    ctx.energy_mgr.subscribe_over_power(rqt)?;

    if let Ok(value) = jsonc.get::<i32>("imax") {
        ctx.energy_mgr.set_imax_cable(value)?;
    }

    if let Ok(value) = jsonc.get::<i32>("pmax") {
        ctx.energy_mgr.set_power_backend(value)?;
    }

    rqt.reply(AFB_NO_DATA, 0);
    Ok(())
}

pub(crate) fn register_verbs(api: &mut AfbApi, config: BindingCfg) -> Result<(), AfbError> {
    const ACTIONS: &str = "['read','subscribe','unsubscribe']";
    const RESET: &str = "['read','subscribe','unsubscribe','reset']";
    const VOLTS: [&str; 4] = ["Volt-Avr", "Volt-L1", "Volt-L2", "Volt-L3"];
    const CURRENTS: [&str; 4] = ["Amp-Total", "Amp-L1", "Amp-L2", "Amp-L3"];
    const POWER: [&str; 4] = ["Watt-Total", "Watt-L1", "Watt-L2", "Watt-L3"];
    const ENERGY: [&str; 2] = ["Energy-Session","Energy-Total"];

    let authorize_verb = AfbVerb::new("authorized-energy")
        .set_name("authorize")
        .set_info("configure max power/current")
        .set_sample("{'imax':10, 'pmax':22}")?
        .set_callback(Box::new(ConfRequestCtx {
            energy_mgr: config.energy_mgr
        }))
        .finalize()?;



    // Tension data_set from eastron modbus meter
    let tension_set = Rc::new(RefCell::new(MeterDataSet::default(MeterTagSet::Tension)));
    let tension_event = AfbEvent::new("tension");
    let tension_verb = AfbVerb::new("tension")
        .set_name("volts")
        .set_info("current tension in volt*100")
        .set_action(ACTIONS)?
        .set_callback(Box::new(MeterRequestCtx {
            data_set: tension_set.clone(),
            labels: &VOLTS,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            evt: tension_event,
        }))
        .finalize()?;

    let tension_handler = AfbEvtHandler::new("tension-evt")
        .set_pattern(to_static_str(format!("{}/Volt*", config.meter_api)))
        .set_callback(Box::new(MeterEvtCtrl {
            data_set: tension_set.clone(),
            evt: tension_event,
            labels: &VOLTS,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            energy_mgr: config.energy_mgr
        }))
        .finalize()?;

    // Energy data_set from eastron modbus meter
    let energy_set = Rc::new(RefCell::new(MeterDataSet::default(MeterTagSet::Energy)));
    let energy_event = AfbEvent::new("energy");
    let energy_verb = AfbVerb::new("energy")
        .set_name("energy")
        .set_info("current energy in volt*100")
        .set_action(RESET)?
        .set_callback(Box::new(MeterRequestCtx {
            data_set: energy_set.clone(),
            labels: &ENERGY,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            evt: energy_event,
        }))
        .finalize()?;

    let energy_handler = AfbEvtHandler::new("energy-evt")
        .set_pattern(to_static_str(format!("{}/Ener*", config.meter_api)))
        .set_callback(Box::new(MeterEvtCtrl {
            data_set: energy_set.clone(),
            evt: energy_event,
            labels: &ENERGY,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            energy_mgr: config.energy_mgr
        }))
        .finalize()?;

    // Current data_set from eastron modbus meter
    let current_set = Rc::new(RefCell::new(MeterDataSet::default(MeterTagSet::Current)));
    let current_event = AfbEvent::new("current");
    let current_verb = AfbVerb::new("current")
        .set_name("amps")
        .set_info("current in amps*100")
        .set_action(ACTIONS)?
        .set_callback(Box::new(MeterRequestCtx {
            data_set: current_set.clone(),
            labels: &CURRENTS,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            evt: current_event,
        }))
        .finalize()?;

    let current_handler = AfbEvtHandler::new("current-evt")
        .set_pattern(to_static_str(format!("{}/Amp*", config.meter_api)))
        .set_callback(Box::new(MeterEvtCtrl {
            data_set: current_set.clone(),
            evt: current_event,
            labels: &VOLTS,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            energy_mgr: config.energy_mgr
        }))
        .finalize()?;

    // Power data_set from eastron modbus meter
    let power_set = Rc::new(RefCell::new(MeterDataSet::default(MeterTagSet::Power)));
    let power_event = AfbEvent::new("power");
    let power_verb = AfbVerb::new("power")
        .set_name("power")
        .set_info("current power in watt*100")
        .set_action(ACTIONS)?
        .set_callback(Box::new(MeterRequestCtx {
            data_set: power_set.clone(),
            labels: &POWER,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            evt: power_event,
        }))
        .finalize()?;

    let power_handler = AfbEvtHandler::new("power-evt")
        .set_pattern(to_static_str(format!("{}/Watt*", config.meter_api)))
        .set_callback(Box::new(MeterEvtCtrl {
            data_set: power_set.clone(),
            evt: power_event,
            labels: &VOLTS,
            meter_api: config.meter_api,
            meter_prefix: "SDM72D",
            energy_mgr: config.energy_mgr,
        }))
        .finalize()?;

    // Over current data_set from Linky meter
    let adps_set = Rc::new(RefCell::new(MeterDataSet::default(
        MeterTagSet::OverCurrent,
    )));
    let adps_event = AfbEvent::new("over-current");
    let adps_verb = AfbVerb::new("over-current")
        .set_name("adps")
        .set_info("current over current(adps) in A")
        .set_action(ACTIONS)?
        .set_callback(Box::new(AdpsRequestCtx {
            data_set: adps_set.clone(),
            linky_api: config.linky_api,
            adps_verb: "ADPS",
            evt: adps_event,
        }))
        .finalize()?;
    let adps_handler = AfbEvtHandler::new("linky-adps-evt")
        .set_pattern(to_static_str(format!("{}/ADPS", config.linky_api)))
        .set_callback(Box::new(LinkyAdpsEvtCtrl {
            data_set: adps_set.clone(),
            evt: adps_event,
            energy_mgr: config.energy_mgr,
        }))
        .finalize()?;

    // register event and verbs
    api.add_event(tension_event);
    api.add_evt_handler(tension_handler);
    api.add_verb(tension_verb);

    api.add_event(energy_event);
    api.add_evt_handler(energy_handler);
    api.add_verb(energy_verb);

    api.add_event(current_event);
    api.add_evt_handler(current_handler);
    api.add_verb(current_verb);

    api.add_event(power_event);
    api.add_evt_handler(power_handler);
    api.add_verb(power_verb);

    api.add_event(adps_event);
    api.add_evt_handler(adps_handler);
    api.add_verb(adps_verb);

    api.add_verb(authorize_verb);

    Ok(())
}
