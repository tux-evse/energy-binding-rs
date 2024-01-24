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
use typesv4::prelude::*;
use std::cell::{RefCell, RefMut, Ref};

pub struct ManagerHandle {
    data_set: RefCell<EnergyState>,
    event: &'static AfbEvent,
    imax: i32,
    pmax: i32,
}

impl ManagerHandle {
    pub fn new(event: &'static AfbEvent, imax: i32, pmax: i32) -> &'static mut Self {
        let handle = ManagerHandle {
            data_set: RefCell::new(EnergyState::default()),
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
    pub fn check_state(&self) -> Result<Ref<'_, EnergyState>, AfbError> {
        match self.data_set.try_borrow() {
            Err(_) => return afb_error!("energy-manager-state", "fail to access &data_set"),
            Ok(value) => Ok(value),
        }
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

        if amp_max!= 0 && amp_max < self.imax {
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

    pub fn set_power_subscription(&self, watt_max: i32) -> Result<&Self, AfbError> {
        let mut data_set = self.get_state()?;

        data_set.subscription_max = watt_max;
        Ok(self)
    }

    pub fn notify_over_power(&self, tag: &MeterTagSet, over_power: i32) -> Result<(), AfbError> {
        afb_log_msg!(
            Notice,
            self.event,
            "Request to stop vehicle power tag:{:?} over-power:{}",
            tag,
            over_power
        );
        self.event.push(false);
        Ok(())
    }

    pub fn subscribe_over_power(&self, rqt: &AfbRequest) -> Result<(), AfbError> {
        self.event.subscribe(rqt)?;
        Ok(())
    }

    pub fn update_data_set(&self, data_new: &MeterDataSet) -> Result<(), AfbError> {
        let mut data_set = self.get_state()?;

        match data_new.tag {
            MeterTagSet::Current => {
                data_set.current= data_new.total;
                if data_new.l1 > data_set.imax
                    || data_new.l2 > data_set.imax
                    || data_new.l3 > data_set.imax
                {
                    self.notify_over_power(&data_new.tag, data_set.imax)?;
                }
            }
            MeterTagSet::Tension => {
                data_set.tension= data_new.total;
                if data_new.l1 > data_set.tension_max
                    || data_new.l2 > data_set.tension_max
                    || data_new.l3 > data_set.tension_max
                {
                    self.notify_over_power(&data_new.tag, data_set.imax)?;
                }
            }
            MeterTagSet::Power => {
                data_set.power= data_new.total;
                if data_new.l1 > data_set.subscription_max
                    || data_new.l2 > data_set.subscription_max
                    || data_new.l3 > data_set.subscription_max
                {
                    self.notify_over_power(&data_new.tag, data_set.subscription_max)?;
                }
                if data_new.l1 > data_set.pmax
                    || data_new.l2 > data_set.pmax
                    || data_new.l3 > data_set.pmax
                {
                    self.notify_over_power(&data_new.tag, data_set.pmax)?;
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
