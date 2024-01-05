#!/bin/bash

DIRNAME=`dirname $0`
export CONTROL_CONFIG_PATH=$DIRNAME
export LD_LIBRARY_PATH=/usr/local/lib64
pkill afb-energy

echo Starting EnergyMgr in debug mode config=$CONTROL_CONFIG_PATH
cynagora-admin set '' 'HELLO' '' '*' yes

afb-binder --name=afb-energy --port=1234 -v --config=$DIRNAME/binding-energy.json
