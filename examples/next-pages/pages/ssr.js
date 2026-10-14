export const getServerSideProps = async () => ({ props: { now: Date.now() } });

export default function Ssr({ now }) {
  return (
    <div className="hero">
      <p>{now}</p>
    </div>
  );
}
