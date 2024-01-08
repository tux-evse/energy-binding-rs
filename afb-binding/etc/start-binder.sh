#!/bin/bash

export LD_LIBRARY_PATH=/usr/local/lib64
pkill afb-energy
cynagora-admin set '' 'HELLO' '' '*' yes
clear

# build test config dirname
DIRNAME=`dirname $0`
cd $DIRNAME/..
CONFDIR=`pwd`/etc

DEVTOOL_PORT=1235
echo Energy debug mode config=$CONFDIR/*.json port=$DEVTOOL_PORT

afb-binder --name=afb-energy --port=$DEVTOOL_PORT -v \
  --config=$CONFDIR/binder-energy.json \
  --config=$CONFDIR/binding-energy.json \
  --config=$CONFDIR/binding-linky.json \
  --config=$CONFDIR/binding-modbus.json \
  --tracereq=all \
  $*