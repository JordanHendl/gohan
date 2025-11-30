namespace miso {
  struct Optional<T> {
    T m_item;
    uint m_present;
    uint3 m_padding;
    bool valid() {
      return m_present != 0;
    }

    T get() {
      return m_item;
    }
  };
}
