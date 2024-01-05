#!/bin/bash

export CONTROL_CONFIG_PATH=$DIRNAME
export LD_LIBRARY_PATH=/usr/local/lib64
pkill afb-energy
clear

DIRNAME=`dirname $0`
DIRNAME=`dirname $DIRNAME`
BASENAME=`basename $DIRNAME -test`
cd $DIRNAME/../$BASENAME
PWD=`pwd`

echo Starting EnergyMgr in debug mode config=$CONTROL_CONFIG_PATH
cynagora-admin set '' 'HELLO' '' '*' yes

afb-binder --name=afb-energy --port=1235 -v \
  --config=$PWD/etc/binder-energy.json \
  --config=$PWD/etc/binding-energy.json \
  --config=$PWD/etc/binding-linky.json \
  --config=$PWD/etc/binding-modbus.json \
  $*