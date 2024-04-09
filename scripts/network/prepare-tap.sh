#!/usr/bin/env bash

# This script helps to prepare an environment for developing berserker network
# workload. There are following preparation steps:
#   * Create and start up a new tun device for berserker to use
#   * Optionally prepare iptables for the device to be visible
#
# The last step is optional, because iptables configuration could be different
# between development environments. Meaning it's not guaranteed this part of
# the script is suitable for every case.

stop() { echo "$*" 1>&2 ; exit 1; }

which ip &>/dev/null || stop "Don't have the ip tool"
which whoami &>/dev/null || stop "Don't have the whoami tool"
which iptables &>/dev/null || stop "Don't have the iptables tool"
which sysctl &>/dev/null || stop "Don't have the sysctl tool"

ADDRESS="192.168.0.1/16"
NAME="tun0"
USER="`whoami`"
CONFIGURE_IPTABLE="false"
CONFIGURE_TUNTAP_IF_EXISTS="false"

while getopts ":a:t:uio" opt; do
  case $opt in
    a) ADDRESS="${OPTARG}"
    ;;
    t) NAME="${OPTARG}"
    ;;
    u) USER="${OPTARG}"
    ;;
    i) CONFIGURE_IPTABLE="true"
    ;;
    o) CONFIGURE_TUNTAP_IF_EXISTS="true"
    ;;
    \?) echo "Invalid option -$OPTARG" >&2
    exit 1
    ;;
  esac
done

echo "Verifying if device ${NAME} is already created..."
if ip tuntap | grep "${NAME}" &> /dev/null;
then
    echo "The devince ${NAME} already exists!"
    if [[ "${CONFIGURE_TUNTAP_IF_EXISTS}" != "true" ]]
    then
        exit 1;
    fi
fi

echo "Creating tun device ${NAME} for user ${USER}..."
ip tuntap add name "${NAME}" mode tun user "${USER}"
ip link set "${NAME}" up

echo "Assigning address ${ADDRESS} to device ${NAME}..."
ip addr add "${ADDRESS}" dev "${NAME}"

if [[ "${CONFIGURE_IPTABLE}" == "true" ]];
then
    echo "Enabling ip forward..."
    sysctl net.ipv4.ip_forward=1

    echo "Preparing iptable..."
    iptables -t nat -A POSTROUTING -s "${ADDRESS}" -j MASQUERADE
    iptables -A FORWARD -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    iptables -A FORWARD -o "${NAME}" -d "${ADDRESS}" -j ACCEPT

    RULE_NR=$(iptables -t filter -L INPUT --line-numbers |\
                grep "REJECT     all" |\
                awk '{print $1}')

    # Excempt tun device from potentiall reject all rule
    if [[ $RULE_NR == "" ]]; then
        iptables -I INPUT -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    else
        iptables -I INPUT $((RULE_NR - 1)) -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    fi
fi
