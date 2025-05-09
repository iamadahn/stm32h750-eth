openocd -f interface/stlink.cfg -f stm32h7x_external_flash.cfg -c init -c halt -c "flash write_image erase app.bin 0x90000000" -c reset -c shutdown
