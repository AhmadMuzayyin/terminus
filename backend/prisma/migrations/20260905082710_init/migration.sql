-- CreateTable
CREATE TABLE `users` (
    `id` VARCHAR(191) NOT NULL,
    `email` VARCHAR(191) NOT NULL,
    `password_hash` VARCHAR(191) NOT NULL,
    `created_at` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),

    UNIQUE INDEX `users_email_key`(`email`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `vaults` (
    `id` VARCHAR(191) NOT NULL,
    `name` VARCHAR(191) NOT NULL,
    `owner_user_id` VARCHAR(191) NOT NULL,
    `created_at` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `vault_members` (
    `vault_id` VARCHAR(191) NOT NULL,
    `user_id` VARCHAR(191) NOT NULL,
    `role` ENUM('owner', 'member') NOT NULL,
    `created_at` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),

    PRIMARY KEY (`vault_id`, `user_id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `host_groups` (
    `id` VARCHAR(191) NOT NULL,
    `vault_id` VARCHAR(191) NOT NULL,
    `name` VARCHAR(191) NOT NULL,
    `subtitle` VARCHAR(191) NULL,
    `parent_id` VARCHAR(191) NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `host_profiles` (
    `id` VARCHAR(191) NOT NULL,
    `vault_id` VARCHAR(191) NOT NULL,
    `label` VARCHAR(191) NOT NULL,
    `host` VARCHAR(191) NOT NULL,
    `port` INTEGER NOT NULL,
    `username` VARCHAR(191) NOT NULL,
    `kind` VARCHAR(191) NOT NULL,
    `auth_method` VARCHAR(191) NOT NULL,
    `credential_id` VARCHAR(191) NULL,
    `group_id` VARCHAR(191) NULL,
    `tags` VARCHAR(191) NOT NULL DEFAULT '[]',
    `terminal_theme` VARCHAR(191) NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `identities` (
    `id` VARCHAR(191) NOT NULL,
    `vault_id` VARCHAR(191) NOT NULL,
    `label` VARCHAR(191) NOT NULL,
    `username` VARCHAR(191) NOT NULL,
    `credential_id` VARCHAR(191) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `secrets` (
    `credential_id` VARCHAR(191) NOT NULL,
    `vault_id` VARCHAR(191) NOT NULL,
    `encrypted_data` LONGBLOB NOT NULL,

    PRIMARY KEY (`credential_id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `refresh_tokens` (
    `id` VARCHAR(191) NOT NULL,
    `user_id` VARCHAR(191) NOT NULL,
    `token_hash` VARCHAR(191) NOT NULL,
    `expires_at` DATETIME(3) NOT NULL,
    `revoked_at` DATETIME(3) NULL,
    `created_at` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),

    UNIQUE INDEX `refresh_tokens_token_hash_key`(`token_hash`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- AddForeignKey
ALTER TABLE `vaults` ADD CONSTRAINT `vaults_owner_user_id_fkey` FOREIGN KEY (`owner_user_id`) REFERENCES `users`(`id`) ON DELETE RESTRICT ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `vault_members` ADD CONSTRAINT `vault_members_vault_id_fkey` FOREIGN KEY (`vault_id`) REFERENCES `vaults`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `vault_members` ADD CONSTRAINT `vault_members_user_id_fkey` FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `host_groups` ADD CONSTRAINT `host_groups_vault_id_fkey` FOREIGN KEY (`vault_id`) REFERENCES `vaults`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `host_profiles` ADD CONSTRAINT `host_profiles_vault_id_fkey` FOREIGN KEY (`vault_id`) REFERENCES `vaults`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `host_profiles` ADD CONSTRAINT `host_profiles_group_id_fkey` FOREIGN KEY (`group_id`) REFERENCES `host_groups`(`id`) ON DELETE SET NULL ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `identities` ADD CONSTRAINT `identities_vault_id_fkey` FOREIGN KEY (`vault_id`) REFERENCES `vaults`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `secrets` ADD CONSTRAINT `secrets_vault_id_fkey` FOREIGN KEY (`vault_id`) REFERENCES `vaults`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `refresh_tokens` ADD CONSTRAINT `refresh_tokens_user_id_fkey` FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;
