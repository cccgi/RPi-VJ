# A3-09 — USB Discovery and Transactional Copy

## Goal

Discover `/RPI-VJ-INBOX/` on a thumb drive and copy candidates safely without
executing or trusting removable content.

## Scope

Use a udev/systemd path trigger only for discovery. Mount with `nosuid,nodev,noexec`
and conservative ownership. Accept common FAT/exFAT/ext4 filesystems. Ignore
symlinks, device files, hidden OS metadata, and files outside the inbox.

For each regular media file, create a job and copy to staging while computing
SHA-256. Detect removal, short reads, duplicate content, filename collisions,
unsupported extension, insufficient disk reserve, and read-only media. Never
transcode directly from the USB drive. Safely unmount after the queue completes.

## Observable

Test normal, read-only, duplicate, full-disk simulation, removal-mid-copy, and
malicious symlink drives. The active show never changes, partial files remain
only in staging, and reinserting the drive resumes without duplicating jobs.

