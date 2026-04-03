#!/bin/bash
# Configure USB HID keyboard gadget via libcomposite
set -euo pipefail

GADGET=/sys/kernel/config/usb_gadget/zerokb

# Tear down existing gadget if present
if [ -d "$GADGET" ]; then
    echo "" > "$GADGET/UDC" 2>/dev/null || true
    rm -f "$GADGET/configs/c.1/hid.usb0"
    rmdir "$GADGET/configs/c.1/strings/0x409" 2>/dev/null || true
    rmdir "$GADGET/configs/c.1" 2>/dev/null || true
    rmdir "$GADGET/functions/hid.usb0" 2>/dev/null || true
    rmdir "$GADGET/strings/0x409" 2>/dev/null || true
    rmdir "$GADGET" 2>/dev/null || true
fi

modprobe libcomposite

mkdir -p "$GADGET"
cd "$GADGET"

echo 0x3434 > idVendor   # Keychron
echo 0x0333 > idProduct  # Keychron V3
echo 0x0102 > bcdDevice
echo 0x0200 > bcdUSB

mkdir -p strings/0x409
echo "" > strings/0x409/serialnumber
echo "Keychron" > strings/0x409/manufacturer
echo "Keychron V3" > strings/0x409/product

mkdir -p configs/c.1/strings/0x409
echo "Keyboard" > configs/c.1/strings/0x409/configuration
echo 500 > configs/c.1/MaxPower

mkdir -p functions/hid.usb0
echo 1 > functions/hid.usb0/protocol      # keyboard
echo 1 > functions/hid.usb0/subclass      # boot interface
echo 8 > functions/hid.usb0/report_length

# Standard USB HID keyboard report descriptor (boot keyboard, 6KRO)
echo -ne '\x05\x01\x09\x06\xa1\x01\x05\x07\x19\xe0\x29\xe7\x15\x00\x25\x01\x75\x01\x95\x08\x81\x02\x95\x01\x75\x08\x81\x03\x95\x05\x75\x01\x05\x08\x19\x01\x29\x05\x91\x02\x95\x01\x75\x03\x91\x03\x95\x06\x75\x08\x15\x00\x25\x65\x05\x07\x19\x00\x29\x65\x81\x00\xc0' \
    > functions/hid.usb0/report_desc

ln -s functions/hid.usb0 configs/c.1/

ls /sys/class/udc > UDC

echo "zerokb HID gadget configured on $(cat UDC)"
