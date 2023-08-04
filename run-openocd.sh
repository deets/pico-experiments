#!/bin/bash
sudo /opt/picoprobe/bin/openocd -f /opt/picoprobe/share/openocd/scripts/interface/cmsis-dap.cfg -f /opt/picoprobe/share/openocd/scripts/target/rp2040.cfg -s tcl
