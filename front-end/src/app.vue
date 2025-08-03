<template>
  <v-app @wheel.prevent @touchmove.prevent @scroll.prevent>
    <v-main>
      <v-container class="mx-auto">

        <ProfileView ref="profileView" class="mt-3" @profileChanged="profileSelected" />

        <OptionCard class="mt-3 opt-card" :cardTitle="'SSBU Settings'"
          :cardSubtitle="'Optimize emulator graphics and CPU settings for SSBU'" :cardDisplayIcon="'mdi-cog'"
          :isOptimized="user_status.settings_optimized && selected_profile != null"
          @updated="(s, o) => { optUpdated('Settings', s, o) }" />
        <OptionCard class="mt-3 opt-card" :cardTitle="'SSBU Mods'"
          :cardSubtitle="'Add useful mods for training and online play'" :cardDisplayIcon="'mdi-folder-wrench'"
          :isOptimized="user_status.mods_optimized && selected_profile != null"
          :advancedOptions="[{ id: 'CleanSkyline', label: 'Clean Skyline Plugins' }, { id: 'CleanArc', label: 'Clean Arcropolis Mods' }]"
          @updated="(s, o) => { optUpdated('Mods', s, o) }" />
        <OptionCard class="mt-3 opt-card" :cardTitle="'Save Data'"
          :cardSubtitle="'Overwrite SSBU save with a 100% save for competitive play'"
          :cardDisplayIcon="'mdi-content-save-all'" :isOptimized="user_status.save_optimized && selected_profile != null"
          @updated="(s, o) => { optUpdated('Save', s, o) }" />

        <v-card-item class="justify-center" style="padding-top: 25px;">
          <v-tooltip location="right" :disabled="selected_profile != null && isAnyOptsEnabled">
            <template v-slot:activator="{ props }">
              <div v-bind="props" class="d-inline-block">
                <v-btn color="primary" :disabled="selected_profile == null || !isAnyOptsEnabled" @click="optimizeSelected">Optimize
                  Selected</v-btn>
              </div>
            </template>
            <span v-if="selected_profile == null">Incorrect Emulator Setup</span>
            <span v-if="!isAnyOptsEnabled">No Option Selected</span>
          </v-tooltip>
        </v-card-item>
      </v-container>
      <div>
        <v-snackbar v-for="(s, i) in snackbars" v-model="s.show" :key="i" :color="s.color" transition="fade-transition"
          :timeout="(s.timeout - 500)" :style="{ 'margin-bottom': calcSnackbarMargin(i) }">
          {{ s.text }}
        </v-snackbar>
      </div>
    </v-main>
  </v-app>
</template>

<script>
import { invoke } from '@tauri-apps/api/core';
import { info, error } from "@tauri-apps/plugin-log";
import { ref } from 'vue';


export default {
  data() {
    return {
      config: null,
      selected_profile: null,
      user_status: {
        settings_optimized: false,
        mods_optimized: false,
        save_optimized: false,
      },
      selected_opts: {
        "Settings": {
          enabled: true,
          options: [],
        },
        "Mods": {
          enabled: true,
          options: [],
        },
        "Save": {
          enabled: true,
          options: [],
        },
      },
      snackbars: [],
    };
  },
  setup() {
    const profileView = ref(null);
    return {
      profileView
    }
  },
  mounted() {
    invoke('query_config').then((c) => {
      this.config = c;
      this.$refs.profileView.init(this.config);
      info('Config Loaded: ' + JSON.stringify(this.config, null, 2));
    }).catch((err) => {
      error(err);
    });
  },
  computed: {
    isAnyOptsEnabled() {
      return (
        this.selected_opts.Settings.enabled ||
        this.selected_opts.Mods.enabled ||
        this.selected_opts.Save.enabled
      )
    }
  },
  methods: {
    updateUserStatus() {
      invoke('get_user_status', { userProfile: this.selected_profile }).then((status) => {
        info('Updated User Status: ' + JSON.stringify(status));
        this.user_status.settings_optimized = status.settings_optimized;
        this.user_status.mods_optimized = status.mods_optimized;
        this.user_status.save_optimized = status.save_optimized;
      }).catch((err) => {
        error(err);
      })
    },
    profileSelected(profile) {
      this.selected_profile = profile;
      this.updateUserStatus()
      info('Selected Profile: ' + JSON.stringify(this.selected_profile));
    },
    optUpdated(key, enabled, options) {
      this.selected_opts[key].enabled = enabled;
      this.selected_opts[key].options = options;
      info('Optimization Updated: ' + JSON.stringify(this.selected_opts));
    },
    optimizeSelected() {
      for (const [key, data] of Object.entries(this.selected_opts)) {
        if (data.enabled) {
          const args = { userProfile: this.selected_profile, optimization: key, advancedOptions: data.options };
          invoke('apply_optimization', args).then(() => {
            info('Optimization Applied: ' + JSON.stringify(args));
            this.showSnackbar('Optimization Applied Successfully: ' + key, 3000, "green");
            this.updateUserStatus();
          }).catch((err) => {
            error(err);
            this.showSnackbar('Error Applying Optimization: ' + key, 3000, "red");
          });
        }
      }
    },
    calcSnackbarMargin(i) {
      return (i * 60) + 'px'
    },
    showSnackbar(message, timeout, color) {
      const snackbar = { show: true, text: message, timeout: timeout, color: color }
      this.snackbars.push(snackbar);
      setTimeout(() => this.hideSnackbar(this.snackbars.length - 1), timeout);
    },
    hideSnackbar(i) {
      this.snackbars.splice(i, 1);
    }
  }
}
</script>

<style>
html {
  overflow: hidden !important;
  scrollbar-width: none;
  -ms-overflow-style: none;
  overscroll-behavior: none;
}

html::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}
</style>
