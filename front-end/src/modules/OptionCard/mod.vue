<template>
  <v-card class="mx-auto opt-card-main" :title="cardTitle" :subtitle="cardSubtitle" @click="toggle">
    <template v-slot:append>
      <StatusIcon :isCorrect="isOptimized" :correctMessage="'Optimized'" :incorrectMessage="'Not Optimized'"
        :location="'left'" />
    </template>
    <template v-slot:prepend>
      <v-icon :icon="cardDisplayIcon"></v-icon>
    </template>
    <v-menu v-if="advancedOptions && advancedOptions.length > 0" v-model="showAdvanced" :close-on-content-click="false"
      location="end">
      <template v-slot:activator="{ props }">
        <v-btn :disabled="!isSelected" color="purple-lighten-2" v-bind="props" append-icon="mdi-chevron-right"
          variant="text" size="small">
          Advanced Options
        </v-btn>
      </template>

      <v-card min-width="300" title="Advanced Options">
        <v-list>
          <v-list-item v-for="option in advancedOptions">
            <v-switch color="purple-lighten-2" v-model="selectedOptions" :value="option.id" :label="option.label"
              @change="optionsUpdated" hide-details>
            </v-switch>
          </v-list-item>
        </v-list>
      </v-card>
    </v-menu>

    <v-checkbox color="purple-lighten-2" class="opt-card-sub" v-model:model-value="isSelected"></v-checkbox>
  </v-card>
</template>

<script>
export default {
  props: {
    cardTitle: String,
    cardSubtitle: String,
    cardDisplayIcon: String,
    isOptimized: Boolean,
    advancedOptions: Array,
  },
  data() {
    return {
      isSelected: true,
      showAdvanced: false,
      selectedOptions: []
    };
  },
  watch: {
    isOptimized: {
      immediate: true, 
      handler (newVal, oldVal) {
        this.isSelected = !newVal;
        this.$emit('updated', this.isSelected, this.selectedOptions);
      }
    }
  },
  methods: {
    toggle() {
      this.isSelected = !this.isSelected;
      if (!this.isSelected) {
        this.showAdvanced = false;
        this.selectedOptions = [];
      }
      this.$emit('updated', this.isSelected, this.selectedOptions);
    },
    optionsUpdated() {
      this.$emit('updated', this.isSelected, this.selectedOptions);
    }
  }
};
</script>

<style scoped>
.opt-card-main {
  position: relative;
  padding: 20px;
  margin: 0px;
}

.opt-card-sub {
  position: absolute;
  top: 0;
  left: 0;
}
</style>
