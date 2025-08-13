<template>
  <v-dialog width="500">
    <template v-slot:activator="{ props }">
      <v-card style="max-width: 300px;" v-bind="props">
        <template v-slot:prepend>
          <v-icon size="large" icon="mdi-account-circle"></v-icon>
        </template>
        <template v-slot:append>
          <StatusIcon :isCorrect="selectedUser != null && emuDataFolder != ''" :correctMessage="`${emuName} Emulator Properly Configured`"
            :incorrectMessage="'Incorrect Emulator Setup'" :location="'right'" />
        </template>
        <template v-slot:title>
          <v-card-title v-if="selectedUser">{{ this.selectedUser.name }}</v-card-title>
          <v-card-title v-if="!selectedUser">???</v-card-title>
        </template>
      </v-card>
    </template>

    <template v-slot:default="{ isActive }">
      <v-card prepend-icon="mdi-account-cog" title="Emulator Setup">
        <v-card-text>
          <v-text-field :disabled="fileDialogOpened" readonly label="Select Emulator Data Folder" v-bind:model-value="emuDataFolder"
            @click.prevent.capture.stop="selectEmuDataFolder">
            <template v-slot:prepend>
              <StatusIcon :isCorrect="emuDataFolder != ''" :correctMessage="`${emuName} Emulator Data Folder Found`"
                :incorrectMessage="'Incorrect Emulator Data Folder'" :location="'top'" />
            </template>
          </v-text-field>
        </v-card-text>

        <v-card-item>
          <v-select v-model="selectedUser" :items="users" item-title="name" label="Select User Profile" return-object
            @update:model-value="profileChanged">
            <template v-slot:prepend>
              <StatusIcon :isCorrect="selectedUser != null" :correctMessage="'User Found'"
                :incorrectMessage="'User Not Found'" :location="'top'" />
            </template>
          </v-select>
        </v-card-item>

        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn text="Return" @click="isActive.value = false"></v-btn>
        </v-card-actions>
      </v-card>
    </template>
  </v-dialog>
</template>


<script>
import { invoke } from '@tauri-apps/api/core';
import { info, error } from "@tauri-apps/plugin-log";

export default {
  data() {
    return {
      config: null,
      fileDialogOpened: false,
      emuName: null,
      emuDataFolder: "",
      users: [],
      selectedUser: null,
    };
  },
  methods: {
    init(config) {
      this.config = config;
      this.users = this.config.user_profiles;
      if (this.config.emu_filesystem.emu_name) {
        this.emuName = this.config.emu_filesystem.emu_name;
      } else {
        this.emuName = null;
      }
      if (this.config.local_data.emu_folder) {
        this.emuDataFolder = this.config.local_data.emu_folder;
      } else {
        this.emuDataFolder = "";
      }
      if (this.config.local_data.selected_user_profile) {
        this.selectedUser = this.config.local_data.selected_user_profile;
      } else {
        if (this.users.length > 0) {
          this.selectedUser = this.users[0];
        } else {
          this.selectedUser = null;
        }
      }
      this.profileChanged(this.selectedUser);
    },
    profileChanged(profile) {
      this.$emit('profileChanged', profile);
      invoke('update_selected_user', { userProfile: profile }).catch((err) => {
        error(err);
        this.selectedUser = null;
      })
    },
    selectEmuDataFolder() {
      if (this.fileDialogOpened) {
        return;
      }
      this.fileDialogOpened = true;
      invoke('select_emu_data_folder').then((config) => {
        this.init(config);
        info('New emulator data folder selected: ' + JSON.stringify(this.config, null, 2));
        this.fileDialogOpened = false;
      }).catch((err) => {
        this.$root.showSnackbar(err, 3000, "red");
        this.fileDialogOpened = false;
        error(err);
      })
    }
  }
}
</script>
