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
use std::cell::{RefCell, RefMut};
use std::time::SystemTime;
use typesv4::prelude::*;

pub struct ManagerHandle {
    data_set: RefCell<EnergyState>,
    event: &'static AfbEvent,
    imax: i32,
    pmax: i32,
}

impl ManagerHandle {
    pub fn new(event: &'static AfbEvent, imax: i32, pmax: i32, umax: i32) -> &'static mut Self {
        let handle = ManagerHandle {
            data_set: RefCell::new(EnergyState::default(imax, pmax, umax)),
            event,
            imax,
            pmax,
        };

        // return a static handle to prevent Rust from complaining when moving/sharing it
        Box::leak(Box::new(handle))
    }

    #[track_caller]
    pub fn get_state(&self) -> Result<RefMut<'_, EnergyState>, AfbError> {
        match self.data_set.try_borrow_mut() {
            Err(_) => return afb_error!("energy-manager-update", "fail to access &mut data_set"),
            Ok(value) => Ok(value),
        }
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
            pmax: data_set.pmax,
            imax: data_set.imax,
        })
    }

    pub fn set_imax_cable(&self, amp_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;

        if amp_max != 0 && amp_max < self.imax {
            data_set.imax = amp_max;
        } else {
            data_set.imax = self.imax;
        }
        Ok(self)
    }

    pub fn set_power_backend(&self, kwh_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;

        if kwh_max != 0 && kwh_max < self.pmax {
            data_set.pmax = kwh_max;
        } else {
            data_set.pmax = self.pmax;
        }
        Ok(self)
    }

    pub fn set_power_subscription(&self, watt_max: i32, volts: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;

        data_set.subscription_max = watt_max*100;
        data_set.volts = volts;
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
    pub fn check_available_current(&self, data: &MeterDataSet) -> Result <(i32,i32), AfbError> {
        let data_set = self.get_state()?;

        let nb_phases = if data.total == data.l1 {
            1
        } else {
            let mut phases= 1;
            if data.l2 > 0 {phases= phases+1};
            if data.l3 > 0 {phases= phases+1};
            phases
        };

        // never use more than 80% of available subscription power
        let remaining= (self.pmax*80)/100 - data.total;
        let iavail= remaining/data_set.volts/nb_phases;

        Ok((iavail/100, data_set.imax)) //move to A
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
                data_set.tension = data_new.l1;
                if data_new.l1 > data_set.umax
                    || data_new.l2 > data_set.umax
                    || data_new.l3 > data_set.umax
                {
                    self.notify_over_power(data_new.tag.clone(), data_set.umax)?;
                }
            }
            MeterTagSet::Power => {
                data_set.power = data_new.total;
                if data_new.l1 > data_set.subscription_max
                    || data_new.l2 > data_set.subscription_max
                    || data_new.l3 > data_set.subscription_max
                {
                    self.notify_over_power(data_new.tag.clone(), data_set.subscription_max)?;
                }
            }

            MeterTagSet::OverCurrent => {
                self.notify_over_power(data_new.tag.clone(), data_set.subscription_max)?;
            }
            _ => {}
        }

        Ok(())
    }
}
