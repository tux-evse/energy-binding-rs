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
use std::cell::RefCell;

pub struct ManagerState {
    subscription_max: i32,
    backend_max: i32,
    tension_max: i32,
    cable_max: i32,
}

impl ManagerState {
    pub fn default() -> Self {
        // Warning: unit are value*100
        ManagerState {
            subscription_max: 900,
            backend_max: 900,
            cable_max: 900,
            tension_max: 25000,
        }
    }
}

pub struct ManagerHandle {
    data_set: RefCell<ManagerState>,
    root_api: AfbApiV4,
    power_api: &'static str,
}

impl ManagerHandle {
    pub fn new(root_api: AfbApiV4, power_api: &'static str) -> &'static mut Self {
        let handle = ManagerHandle {
            data_set: RefCell::new(ManagerState::default()),
            power_api,
            root_api,
        };

        // return a static handle to prevent Rust from complaining when moving/sharing it
        Box::leak(Box::new(handle))
    }

    pub fn set_imax_cable(&self, amp_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = match self.data_set.try_borrow_mut() {
            Err(_) => return afb_error!("energy-manager-update", "fail to access &mut data_set"),
            Ok(value) => value,
        };

        data_set.cable_max = amp_max;
        Ok(self)
    }

    pub fn set_power_backend(&self, kwh_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = match self.data_set.try_borrow_mut() {
            Err(_) => return afb_error!("energy-manager-update", "fail to access &mut data_set"),
            Ok(value) => value,
        };

        data_set.backend_max = kwh_max;
        Ok(self)
    }

    pub fn set_power_subscription(&self, watt_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = match self.data_set.try_borrow_mut() {
            Err(_) => return afb_error!("energy-manager-update", "fail to access &mut data_set"),
            Ok(value) => value,
        };

        data_set.subscription_max = watt_max;
        Ok(self)
    }

    pub fn notify_over_power(&self, tag: &MeterTagSet, over_power: i32) -> Result<(), AfbError> {
        afb_log_msg!(
            Notice,
            self.root_api,
            "Request to stop vehicle power tag:{:?} over-power:{}",
            tag,
            over_power
        );
        AfbSubCall::call_sync(self.root_api, self.power_api, "power", false)?;
        Ok(())
    }

    pub fn update_data_set(&self, data_new: &MeterDataSet) -> Result<(), AfbError> {
        let data_set = match self.data_set.try_borrow() {
            Err(_) => return afb_error!("energy-manager-update", "fail to access &data_set"),
            Ok(value) => value,
        };

        match data_new.tag {
            MeterTagSet::Current => {
                if data_new.l1 > data_set.cable_max
                    || data_new.l2 > data_set.cable_max
                    || data_new.l3 > data_set.cable_max
                {
                    self.notify_over_power(&data_new.tag, data_set.cable_max)?;
                }
            }
            MeterTagSet::Tension => {
                if data_new.l1 > data_set.tension_max
                    || data_new.l2 > data_set.tension_max
                    || data_new.l3 > data_set.tension_max
                {
                    self.notify_over_power(&data_new.tag, data_set.cable_max)?;
                }
            }
            MeterTagSet::Power => {
                if data_new.l1 > data_set.subscription_max
                    || data_new.l2 > data_set.subscription_max
                    || data_new.l3 > data_set.subscription_max
                {
                    self.notify_over_power(&data_new.tag, data_set.subscription_max)?;
                }
                if data_new.l1 > data_set.backend_max
                    || data_new.l2 > data_set.backend_max
                    || data_new.l3 > data_set.backend_max
                {
                    self.notify_over_power(&data_new.tag, data_set.backend_max)?;
                }
            }
            MeterTagSet::OverCurrent => {
                self.notify_over_power(&data_new.tag, data_set.subscription_max)?;
            }
            _ => {}
        }

        Ok(())
    }
}
