BIN_DIR=../target/armv7-unknown-linux-gnueabihf/debug
mv systemd_files/* /etc/systemd/system/
mv $BIN_DIR/api_server /usr/local/bin/
mv $BIN_DIR/desktopper /usr/local/bin/
systemctl daemon-reload
systemctl enable desktopper task_api desktopper_display
systemctl start desktopper
