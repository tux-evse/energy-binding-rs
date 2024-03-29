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

use afbv4::prelude::*;
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;
use typesv4::prelude::*;

pub struct ManagerHandle {
    data_set: Mutex<EnergyState>,
    event: &'static AfbEvent,
    imax: i32,
    pmax: i32,
    phase: i32,
}

impl ManagerHandle {
    pub fn new(event: &'static AfbEvent, imax: i32, pmax: i32, umax: i32, phase: i32) -> &'static mut Self {
        let imax = imax * 1000;
        let pmax = pmax * 1000;
        let umax = umax * 1000;
        let handle = ManagerHandle {
            data_set: Mutex::new(EnergyState::default(imax, pmax, umax)),
            event,
            imax: imax,
            pmax: pmax,
            phase,
        };

        // return a static handle to prevent Rust from complaining when moving/sharing it
        Box::leak(Box::new(handle))
    }

    #[track_caller]
    pub fn get_state(&self) -> Result<MutexGuard<'_, EnergyState>, AfbError> {
        let guard = self.data_set.lock().unwrap();
        Ok(guard)
    }

    #[track_caller]
    pub fn clone_state(&self) -> Result<EnergyState, AfbError> {
        let unix_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(value) => value,
            Err(_) => return afb_error!("energy-check-state", "system time before UNIX EPOCH!"),
        };

        let mut data_set = self.get_state()?;
        data_set.timestamp = unix_time;
        Ok(data_set.clone())
    }

    #[track_caller]
    pub fn get_config(&self) -> Result<EngyConfSet, AfbError> {
        let data_set = self.get_state()?;
        Ok(EngyConfSet {
            pmax: data_set.pmax / 1000,
            imax: data_set.imax / 1000,
        })
    }

    pub fn set_imax_cable(&self, amp_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;
        let amp_max = amp_max * 1000;
        if amp_max != 0 && amp_max < self.imax {
            data_set.imax = amp_max;
        } else {
            data_set.imax = self.imax;
        }
        Ok(self)
    }

    pub fn set_power_backend(&self, kwh_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;
        let kwh_max = kwh_max * 1000;

        if kwh_max != 0 && kwh_max < self.pmax {
            data_set.pmax = kwh_max;
        } else {
            data_set.pmax = self.pmax;
        }
        Ok(self)
    }

    pub fn set_power_subscription(&self, watt_max: i32, volts: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;

        data_set.subscription_max = watt_max * 1000;
        data_set.tension= volts;
        Ok(self)
    }

    pub fn notify_over_power(&self, tag: MeterTagSet, over_power: i32) -> Result<(), AfbError> {
        afb_log_msg!(
            Notice,
            self.event,
            "Request to stop vehicle power tag:{:?} over-power:{}",
            tag,
            over_power
        );
        self.event.push(tag);
        Ok(())
    }

    // TBD fulup: make available current per phase smarter
    pub fn check_available_current(&self, data: &MeterDataSet) -> Result<(i32, i32), AfbError> {
        let data_set = self.get_state()?;

        // never use more than 80% of available subscription power
        let remaining = (self.pmax * 80) / 100 - data.total;
        let iavail = remaining / data_set.tension / self.phase;
        Ok((iavail, data_set.imax)) //move to A/phase
    }

    pub fn subscribe_over_power(&self, rqt: &AfbRequest) -> Result<(), AfbError> {
        self.event.subscribe(rqt)?;
        Ok(())
    }

    pub fn check_over_subscription(&self, data_new: &MeterDataSet) -> Result<(), AfbError> {
        let mut data_set = self.get_state()?;

        match data_new.tag {
            MeterTagSet::Current => {
                data_set.current = data_new.total;
                if data_new.l1 > data_set.imax
                    || data_new.l2 > data_set.imax
                    || data_new.l3 > data_set.imax
                {
                    self.notify_over_power(data_new.tag.clone(), data_set.imax)?;
                }
            }
            MeterTagSet::Tension => {
                data_set.tension = data_new.total;
                if data_new.l1 > data_set.umax
                    || data_new.l2 > data_set.umax
                    || data_new.l3 > data_set.umax
                {
                    self.notify_over_power(data_new.tag.clone(), data_set.umax)?;
                }
            }
            MeterTagSet::Power => {
                data_set.power = data_new.total;
                if data_new.total > data_set.subscription_max*1000 // Power is in wath subscription in kW
                {
                    self.notify_over_power(data_new.tag.clone(), data_set.subscription_max)?;
                }
            }

            MeterTagSet::Energy => {
                data_set.session = data_new.total;
            }

            MeterTagSet::OverCurrent => {
                self.notify_over_power(data_new.tag.clone(), data_set.subscription_max)?;
            }
            _ => {}
        }

        Ok(())
    }
}
