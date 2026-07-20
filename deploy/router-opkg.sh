#!/bin/sh
# newifi3 (mipsel) 基础安装
opkg update
opkg install lighttpd lighttpd-mod-mbedtls curl ca-bundle
