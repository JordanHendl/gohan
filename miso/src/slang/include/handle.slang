struct Handle {
  uint m_id;
  uint m_gen;

  bool valid() {
    return m_id != 0xFFFFu;
  }

  int id() {
    return int(m_id);
  }
};
