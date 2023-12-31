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
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub enum MeterTagSet {
    Current,
    Tension,
    Power,
    OverCurrent,
    Energy,
    #[default]
    Unset,
}

// hole meter in 00.A value
AfbDataConverter!(meter_data_set, MeterDataSet);
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct MeterDataSet {
    #[serde(skip_serializing)]
    pub start: i32,
    #[serde(skip_serializing)]
    pub variation: i32,
    #[serde(skip_serializing)]
    pub updated: bool,
    pub tag: MeterTagSet,
    pub total: i32,
    pub l1: i32,
    pub l2: i32,
    pub l3: i32,
}

impl MeterDataSet {
    pub fn default(tag: MeterTagSet) -> Self {
        MeterDataSet {
            tag: tag,
            variation: 1,
            start: 0,
            updated: false,
            total: 0,
            l1: 0,
            l2: 0,
            l3: 0,
        }
    }

    // update data_set and set updated flag when total changes.
    pub fn update(&mut self, phase: usize, meter: f64) -> Result<(), AfbError> {
        let value = (meter * 100.0).round() as i32;
        match phase {
            0 => {
                let value = value - self.start; // special reset counter
                if self.total * 100 / self.variation < value
                    || value > self.l3 * 100 / self.variation
                {
                    self.total = value;
                    self.updated = true;
                }
            }
            1 => {
                if self.l1 * 100 / self.variation < value || value > self.l3 * 100 / self.variation
                {
                    self.l1 = value;
                }
            }
            2 => {
                if self.l2 * 100 / self.variation < value || value > self.l3 * 100 / self.variation
                {
                    self.l3 = value;
                }
            }
            3 => {
                if self.l2 * 100 / self.variation < value || value > self.l3 * 100 / self.variation
                {
                    self.l3 = value;
                }
            }
            _ => return afb_error!("data-set-update", "invalid phase:{}", phase),
        }
        Ok(())
    }
}

AfbDataConverter!(power_events, PowerEvent);
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase", tag = "action")]
pub enum PowerEvent {
    #[default]
    STOP,
    START,
    IMAX(u32),
    UNSET,
}


pub fn engy_registers() -> Result<(), AfbError> {
    // add binding custom converter
    meter_data_set::register()?;
    power_events::register()?;
    Ok(())
}
