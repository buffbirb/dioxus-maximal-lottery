variable "oci_compartment_ocid" {
  description = "OCID of the compartment to create resources in"
}

variable "ssh_public_key" {
  description = "SSH public key contents for VM access (e.g. ssh-ed25519 AAAA...)"
}

variable "github_repository" {
  description = "GitHub repository in <owner>/<repo> format"
}

variable "github_runner_pat" {
  description = "GitHub fine-grained PAT with administration:read scope"
  sensitive   = true
}
