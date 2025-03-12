#!/bin/bash -e

if ! [ -f ./mnt/EFI/BOOT/BOOTX64.EFI ] ; then
    echo 'BOOTX64.EFI is not found in ./mnt.'
    echo 'Please run `cargo run` once to generate it and try again.'
fi

if cat /opt/google/cros-containers/etc/lsb-release | grep 'Chrome OS' ; then
    # For crostini (Linux environment on ChromeOS)
    if ls -lahd /mnt/chromeos/removable/WASABIOS ; then
        cp -r ./mnt/* /mnt/chromeos/removable/WASABIOS/
        echo 'Done!'
        echo 'Please unmount from the File app first then remove the disk!'
    else
        echo 'Disk "WASABIOS" not found under /mnt/chromeos/removable/.'
        echo 'Please insert a disk and share it with Linux from the File app.'
        exit 1
    fi
else
    # For bare-metal Linux environment
    DISK=`readlink -f /dev/disk/by-partlabel/WASABIOS`
    echo "Write WasabiOS to ${DISK}. Are you sure?"
    read -p "[Enter to proceed, or Ctrl-C to abort] " REPLY
    mkdir -p ./usb
    sudo mount ${DISK} ./usb
    sudo cp -r mnt/* ./usb
    sudo umount ${DISK}
fi
