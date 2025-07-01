#!/usr/bin/env bash
set -eou pipefail

# This script helps to prepare an environment for developing berserker network
# workload. It has the following preparatory steps:
#   * Create and start up a new tun device for berserker to use
#   * Optionally prepare iptables for the device to be visible
#
# The last step is optional, because iptables configuration could be different
# between development environments. Meaning it's not guaranteed that this part of
# the script is suitable for every case.

stop() { echo "$*" 1>&2 ; exit 1; }

which ip &>/dev/null || stop "Don't have the ip tool"
which whoami &>/dev/null || stop "Don't have the whoami tool"
which sysctl &>/dev/null || stop "Don't have the sysctl tool"

ADDRESS="10.0.0.1/16"
NAME="berserker0"
USER="$(whoami)"
CONFIGURE_IPTABLE="false"
CONFIGURE_FIREWALLD="false"
CONFIGURE_TUNTAP_IF_EXISTS="false"

while getopts ":a:t:u:ifo" opt; do
  case $opt in
    a) ADDRESS="${OPTARG}"
    ;;
    t) NAME="${OPTARG}"
    ;;
    u) USER="${OPTARG}"
    ;;
    i) CONFIGURE_IPTABLE="true"
    ;;
    f) CONFIGURE_FIREWALLD="true"
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

    ip link delete "${NAME}"
fi

echo "Creating tun device ${NAME} for user ${USER}..."
ip tuntap add name "${NAME}" mode tun user "${USER}"
ip link set "${NAME}" up

echo "Assigning address ${ADDRESS} to device ${NAME}..."
ip addr add "${ADDRESS}" dev "${NAME}"

echo "Enabling ip forward..."
sysctl net.ipv4.ip_forward=1

if [[ "${CONFIGURE_FIREWALLD}" == "true" ]];
then
    which firewall-cmd &>/dev/null || stop "Don't have the firewal-cmd tool"

    echo "Adding to the trusted zone..."
    firewall-cmd --zone=trusted --add-interface="${NAME}" || true
fi

echo "${CONFIGURE_IPTABLE}"
if [[ "${CONFIGURE_IPTABLE}" == "true" ]];
then
    IPTABLES=iptables
    if command -v iptables-nft &> /dev/null; then
      IPTABLES=iptables-nft
    fi

    which "${IPTABLES}" &>/dev/null || stop "Don't have the iptables tool"

    echo "Preparing iptable..."
    "${IPTABLES}" -t nat -A POSTROUTING -s "${ADDRESS}" -j MASQUERADE
    "${IPTABLES}" -A FORWARD -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    "${IPTABLES}" -A FORWARD -o "${NAME}" -d "${ADDRESS}" -j ACCEPT

    RULE_NR=$("${IPTABLES}" -t filter -L INPUT --line-numbers |\
                grep "REJECT     all" |\
                awk '{print $1}')

    # Excempt tun device from potentiall reject all rule
    if [[ $RULE_NR == "" ]]; then
        iptables-nft -I INPUT -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    else
        iptables-nft -I INPUT $((RULE_NR - 1)) -i "${NAME}" -s "${ADDRESS}" -j ACCEPT
    fi
fi
