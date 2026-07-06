mod memory;

pub use memory::MemoryOptimizer;

// Re-export optimization primitives from the browser-optimizations crate.
pub use rv8_browser_optimizations::cache::{
    CacheStats, CachedResource, DiskCache, DiskCacheEntry, DiskCacheStats, LruCache, ResourceKey,
    TextureAtlas, TextureAtlasManager,
};
pub use rv8_browser_optimizations::memory::{
    DataType, DiskStorage, DiskStorageStats, FrozenTabState, MemoryPressureLevel,
    MemoryPressureMonitor, ResidencyState, SystemMemoryInfo, TabResidency, TabResidencyManager,
    TabSnapshot, TabStats,
};
pub use rv8_browser_optimizations::network::{
    DnsPrefetchCache, PrefetchManager, PrefetchPriority, PrefetchRequest, PriorityQueue,
    ResourceKind, ResourcePriority, ResourceRequest, ResourceType,
};
pub use rv8_browser_optimizations::runtime::{
    EngineRuntime, InputEvent, LifecycleEvent, PlatformTier, RuntimeError, SafeAreaInsets,
    SurfaceDescriptor, SurfaceId, SurfaceRotation, SurfaceSize,
};
