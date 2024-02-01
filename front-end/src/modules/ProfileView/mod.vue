<template>
  <v-dialog width="500">
    <template v-slot:activator="{ props }">
      <v-card style="max-width: 300px;" v-bind="props">
        <template v-slot:prepend>
          <v-icon size="large" icon="mdi-account-circle"></v-icon>
        </template>
        <template v-slot:append>
          <StatusIcon :isCorrect="selectedUser != null && yuzuDataFolder != ''" :correctMessage="'Good to go!'"
            :incorrectMessage="'Incorrect yuzu setup'" :location="'right'" />
        </template>
        <template v-slot:title>
          <v-card-title v-if="selectedUser">{{ this.selectedUser.name }}</v-card-title>
          <v-card-title v-if="!selectedUser">???</v-card-title>
        </template>
      </v-card>
    </template>

    <template v-slot:default="{ isActive }">
      <v-card prepend-icon="mdi-account-cog" title="Yuzu Setup">
        <v-card-text>
          <v-text-field :disabled="fileDialogOpened" readonly label="Select yuzu Data Folder" v-bind:model-value="yuzuDataFolder"
            @click.prevent.capture.stop="selectYuzuDataFolder">
            <template v-slot:prepend>
              <StatusIcon :isCorrect="yuzuDataFolder != ''" :correctMessage="'yuzu Folder Found'"
                :incorrectMessage="'Incorrect yuzu Folder'" :location="'top'" />
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
import { invoke } from '@tauri-apps/api/tauri';
import { info, error } from "tauri-plugin-log-api";

export default {
  data() {
    return {
      config: null,
      fileDialogOpened: false,
      yuzuDataFolder: "",
      users: [],
      selectedUser: null,
    };
  },
  methods: {
    init(config) {
      this.config = config;
      this.users = this.config.user_profiles;
      if (this.config.local_data.yuzu_folder) {
        this.yuzuDataFolder = this.config.local_data.yuzu_folder;
      } else {
        this.yuzuDataFolder = "";
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
    selectYuzuDataFolder() {
      if (this.fileDialogOpened) {
        return;
      }
      this.fileDialogOpened = true;
      invoke('select_yuzu_data_folder').then((config) => {
        this.init(config);
        info('New data folder selected: ' + JSON.stringify(this.config, null, 2));
        this.fileDialogOpened = false;
      }).catch((err) => {
        this.fileDialogOpened = false;
        error(err);
      })
    }
  }
}
</script>
