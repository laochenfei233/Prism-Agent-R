<script lang="ts">
	let { size = 24 }: { size?: number } = $props();
	let svg = $derived(`<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" fill="none" viewBox="9 9 102 102" >
      <path
        fill="url(#baichuan-i-baichuanlight__a)"
        d="M43.2766 24.1665H31.8099L24.6432 39.6931V80.7832L17 95.8332H35.6333L42.9003 80.7832L43.2766 24.1665ZM69.5567 24.1665H50.9234V95.8332H69.5567V24.1665ZM77.2 44.7098H95.8333V95.8332H77.2V44.7098ZM95.8333 24.1665H77.2V38.9764H95.8333V24.1665Z"
      />
      <defs>
        <linearGradient
          id="baichuan-i-baichuanlight__a"
          x1="31.004"
          x2="89.645"
          y1="30.386"
          y2="95.212"
          gradient-units="userSpaceOnUse">
          <stop stop-color="#FEC13E" />
          <stop offset="1" stop-color="#FF6933" />
        </linearGradient>
      </defs>
    </svg>`);
</script>


{@html svg}
