#!/bin/bash

DIRNAME=`dirname $0`
export CONTROL_CONFIG_PATH=$DIRNAME
export LD_LIBRARY_PATH=/usr/local/lib64
pkill afb-energy

echo Starting EnergyMgr in debug mode config=$CONTROL_CONFIG_PATH
#cynagora-admin set '' 'HELLO' '' '*' yes
cp --update /home/fulup/Workspace/modbus-binding/build/src/afb-modbus.so /usr/redpesk/modbus-binding/lib/.
cp --update /home//fulup/.cargo/build/debug/libafb_linky.so /usr/redpesk/linky-binding-rs/lib/.
cp --update /home//fulup/.cargo/build/debug/libafb_energy.so /usr/redpesk/energy-binding-rs/lib/.

afb-binder --name=afb-energy --port=1234 -v --config=$DIRNAME/binding-energy.json
