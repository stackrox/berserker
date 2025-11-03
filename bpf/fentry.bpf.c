#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

char LICENSE[] SEC("license") = "Dual MIT/GPL";

SEC("fentry/XXX")
int BPF_PROG(fentry_XXX)
{
	pid_t pid;

	pid = bpf_get_current_pid_tgid() >> 32;
	bpf_printk("fentry: pid = %d\n", pid);
	return 0;
}

SEC("fexit/XXX")
int BPF_PROG(fexit_XXX)
{
	pid_t pid;

	pid = bpf_get_current_pid_tgid() >> 32;
	bpf_printk("fexit: pid = %d\n", pid);
	return 0;
}
