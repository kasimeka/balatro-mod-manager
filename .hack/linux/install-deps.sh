#!/usr/bin/env sh
set -xe

apt-get update && apt-get install -y \
  curl \
  unzip \
  \
  pkg-config \
  file \
  xdg-utils \
  \
  libssl-dev \
  libglib2.0-dev \
  libgtk-3-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev=2.44.0-2 \
  libjavascriptcoregtk-4.1-0=2.44.0-2 \
  gir1.2-javascriptcoregtk-4.1=2.44.0-2 \
  libwebkit2gtk-4.1-dev=2.44.0-2 \
  libwebkit2gtk-4.1-0=2.44.0-2 \
  gir1.2-webkit2-4.1=2.44.0-2
