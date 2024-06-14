JH_DIR=~/jailhouse-arceos
JH=$JH_DIR/tools/jailhouse

echo "create axvm arceos"
sudo $JH axvm create 2 1 ./arceos-bios.bin ./arceos.bin