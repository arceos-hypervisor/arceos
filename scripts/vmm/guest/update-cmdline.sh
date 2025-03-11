# Update grub config
cmdline='memmap=0x8000000\\\\\\$0x40000000'
sudo sed -i "s/GRUB_CMDLINE_LINUX=.*/GRUB_CMDLINE_LINUX=$cmdline/" /etc/default/grub
echo "Appended kernel cmdline: $cmdline, see '/etc/default/grub'"
