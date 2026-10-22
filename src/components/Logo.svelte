<script lang="ts">
  // The VC Alert logo (assets/appicon.svg, the same glyph as the app icon).
  // Decorative wherever it appears, so screen readers skip it.
  import logo from '../../assets/appicon.svg';
  let { size = 48, class: className = '' }: { size?: number; class?: string } = $props();
</script>

<img class="logo {className}" src={logo} alt="" width={size} height={size} draggable="false" />

<style>
  .logo {
    display: block;
    flex: none;
    user-select: none;
  }
</style>
