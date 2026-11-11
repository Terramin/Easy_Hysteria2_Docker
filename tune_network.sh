#!/bin/bash
set -e

sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
sudo sysctl -w net.core.default_qdisc=fq
sudo sysctl -w net.ipv4.tcp_congestion_control=bbr

# Permanent settings (идемпотентно: удаляем старые записи перед добавлением)
for key in net.core.rmem_max net.core.wmem_max net.core.default_qdisc net.ipv4.tcp_congestion_control net.core.netdev_max_backlog; do
    sudo sed -i "/^${key}=/d" /etc/sysctl.conf
done

sudo tee -a /etc/sysctl.conf > /dev/null << EOF
net.core.rmem_max=16777216
net.core.wmem_max=16777216
net.core.default_qdisc=fq
net.ipv4.tcp_congestion_control=bbr
net.core.netdev_max_backlog=2000
EOF
sudo sysctl -p
