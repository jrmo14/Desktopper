BIN_DIR=../target/armv7-unknown-linux-gnueabihf/debug
echo "Stopping/Removing job"
systemctl stop desktopper task_api desktopper_display
systemctl disable desktopper task_api desktopper_display
echo "Moving files"
mv systemd_files/* /etc/systemd/system/
mkdir -p /etc/desktopper
mv config.toml /etc/desktopper
mv $BIN_DIR/api_server /usr/local/bin/
mv $BIN_DIR/desktopper /usr/local/bin/
killall desktopper_display
echo "Reloading service files"
systemctl daemon-reload
echo "Starting service"
systemctl enable desktopper task_api desktopper_display
systemctl start desktopper
